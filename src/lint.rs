use std::collections::BTreeSet;

use polka::{Chunk, Module, OpCode};

#[derive(Debug, Clone)]
pub struct PosaraLint {
    pub code: &'static str,
    pub line: Option<usize>,
    pub message: String,
}

impl PosaraLint {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, line: None, message: message.into() }
    }

    pub fn with_line(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }

    pub fn pretty_print(&self) -> String {
        match self.line {
            Some(l) => format!("warning[{}] at line {}: {}", self.code, l, self.message),
            None    => format!("warning[{}]: {}", self.code, self.message),
        }
    }
}

#[cfg(feature = "compiler")]
pub fn from_abrase_lint(w: &abrase::lint::Lint) -> PosaraLint {
    let mut p = PosaraLint::new(w.code, &w.message);
    if w.span.line > 0 { p = p.with_line(w.span.line); }
    p
}

const SCREEN_CONFIG_PORT: u64 = 0x2000;
const SCREEN_COMMIT_PORT: u64 = 0x2001;

const DRAW_NATIVES: &[&str] = &[
    "cls", "pset", "rect", "rectb", "rectmix", "dither", "line", "linew",
    "circ", "circb", "tri", "trib", "pal", "blit", "blitg", "blitr", "sprite", "save_png",
];

fn known_device(id: u8) -> bool {
    matches!(id, 0x00 | 0x10 | 0x20 | 0x80 | 0x90 | 0xE0 | 0xE1 | 0xE2)
}

// first-seen presence of a fact, with its source line when debug info survives.
#[derive(Default, Clone)]
struct Fact { hit: bool, line: Option<usize> }
impl Fact {
    fn set(&mut self, line: Option<usize>) {
        if !self.hit { self.hit = true; self.line = line; }
    }
}

#[derive(Default, Clone)]
struct FnFacts {
    callees: Vec<usize>,
    opens_screen: Fact,
    commits: Fact,
    draws: Fact,
    screen_sites: Vec<Option<usize>>,
}

// transitive closure of `seed` over the call graph.
fn reachable(seed: &[usize], facts: &[FnFacts]) -> BTreeSet<usize> {
    let mut seen: BTreeSet<usize> = BTreeSet::new();
    let mut stack: Vec<usize> = seed.iter().copied().collect();
    while let Some(f) = stack.pop() {
        if seen.insert(f) {
            if let Some(ff) = facts.get(f) {
                for &c in &ff.callees { if !seen.contains(&c) { stack.push(c); } }
            }
        }
    }
    seen
}

pub fn lint_module(module: &Module) -> Vec<PosaraLint> {
    let native_id = |want: &str| -> Option<usize> {
        module.functions.iter().enumerate().find_map(|(i, c)| match c {
            Chunk::Native(n) if n.name == want => Some(i),
            _ => None,
        })
    };
    let screen_id = native_id("screen");
    let screen_off_id = native_id("screen_off");
    let draw_ids: BTreeSet<usize> = module.functions.iter().enumerate().filter_map(|(i, c)| match c {
        Chunk::Native(n) if DRAW_NATIVES.contains(&n.name.as_str()) => Some(i),
        _ => None,
    }).collect();
    let update_fid = module.exports.iter().find(|e| e.name == "update").map(|e| e.fn_id as usize);
    let start_fid = module.exports.iter().find(|e| e.name == "start").map(|e| e.fn_id as usize);

    let mut warns: Vec<PosaraLint> = Vec::new();
    let mut facts: Vec<FnFacts> = vec![FnFacts::default(); module.functions.len()];
    let mut bad_ports: Vec<(u64, Option<usize>)> = Vec::new();
    let mut seen_ports: BTreeSet<u64> = BTreeSet::new();

    for (fidx, chunk) in module.functions.iter().enumerate() {
        let Chunk::Bytecode(bc) = chunk else { continue };
        let line_at = |oi: usize| -> Option<usize> {
            match bc.lines.get(oi).copied() { Some(l) if l > 0 => Some(l as usize), _ => None }
        };
        let mut reg_const: [Option<u64>; 256] = [None; 256];
        let mut seen_screen_here = false;
        for (oi, op) in bc.code.iter().enumerate() {
            match op {
                OpCode::PushConst(r, idx) => reg_const[r.0 as usize] = bc.constants.get(*idx as usize).copied(),
                OpCode::Move(d, s) | OpCode::Copy(d, s) => reg_const[d.0 as usize] = reg_const[s.0 as usize],
                OpCode::Call(_, fid) => {
                    let fid = *fid as usize;
                    facts[fidx].callees.push(fid);
                    if Some(fid) == screen_id {
                        facts[fidx].screen_sites.push(line_at(oi));
                        facts[fidx].opens_screen.set(line_at(oi));
                        seen_screen_here = true;
                    }
                    if draw_ids.contains(&fid) { facts[fidx].draws.set(line_at(oi)); }
                    if Some(fid) == screen_off_id && seen_screen_here {
                        let mut w = PosaraLint::new("screen_order",
                            "screen_off() called after screen() in the same fn (must precede it)");
                        if let Some(l) = line_at(oi) { w = w.with_line(l); }
                        warns.push(w);
                    }
                    reg_const = [None; 256];
                }
                OpCode::Dei(_, p) | OpCode::Deo(_, p) => {
                    if let Some(port) = reg_const[p.0 as usize] {
                        match port {
                            SCREEN_CONFIG_PORT => facts[fidx].opens_screen.set(line_at(oi)),
                            SCREEN_COMMIT_PORT => facts[fidx].commits.set(line_at(oi)),
                            _ => {}
                        }
                        let id = ((port >> 8) & 0xFF) as u8;
                        if port <= 0xFFFF && !known_device(id) && seen_ports.insert(port) {
                            bad_ports.push((port, line_at(oi)));
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // reachability: screen/draw from any entry; commit must be reachable from update.
    let mut any_seed: Vec<usize> = vec![module.entry];
    if let Some(s) = start_fid { any_seed.push(s); }
    if let Some(u) = update_fid { any_seed.push(u); }
    let reach_any = reachable(&any_seed, &facts);
    let reach_update = update_fid.map(|u| reachable(&[u], &facts)).unwrap_or_default();

    let first = |set: &BTreeSet<usize>, pick: &dyn Fn(&FnFacts) -> &Fact| -> Option<(usize, Option<usize>)> {
        set.iter().filter_map(|&f| {
            let fa = pick(&facts[f]);
            if fa.hit { Some((f, fa.line)) } else { None }
        }).next()
    };

    let opens = first(&reach_any, &|f| &f.opens_screen);
    let draws = first(&reach_any, &|f| &f.draws);
    let commit_in_update = first(&reach_update, &|f| &f.commits);

    // screen() must be called exactly once, and not from update().
    let screen_sites: usize = reach_any.iter().map(|&f| facts[f].screen_sites.len()).sum();
    if screen_sites > 1 {
        let mut w = PosaraLint::new("screen_multi_call",
            format!("screen() called from {screen_sites} sites; it must be called exactly once"));
        if let Some((_, Some(l))) = opens { w = w.with_line(l); }
        warns.push(w);
    }
    if update_fid.is_some() {
        let screen_in_update = reach_update.iter()
            .flat_map(|&f| facts[f].screen_sites.iter().copied())
            .next();
        if let Some(line) = screen_in_update {
            let mut w = PosaraLint::new("screen_in_update",
                "screen() reachable from update(); call it once in start()");
            if let Some(l) = line { w = w.with_line(l); }
            warns.push(w);
        }
    }

    if update_fid.is_some() && opens.is_some() && commit_in_update.is_none() {
        let mut w = PosaraLint::new("missing_commit_frame",
            "opens a screen but update() never commits it (device_in(0x2001,1)); window stays blank");
        if let Some((_, Some(l))) = opens { w = w.with_line(l); }
        warns.push(w);
    }
    if draws.is_some() && opens.is_none() {
        let mut w = PosaraLint::new("draw_without_screen",
            "draws but never opens a screen (screen()/device_in(0x2000,..)); framebuffer is 0x0, nothing shows");
        if let Some((_, Some(l))) = draws { w = w.with_line(l); }
        warns.push(w);
    }
    for (p, line) in &bad_ports {
        let mut w = PosaraLint::new("device_port_out_of_range",
            format!("device port {:#06x} targets unknown device {:#04x} (typo?)", p, (p >> 8) & 0xFF));
        if let Some(l) = line { w = w.with_line(*l); }
        warns.push(w);
    }
    warns
}


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

const SCREEN_CONFIG_PORT: u64 = crate::devices::port(crate::devices::SCREEN, 0x00);
const SCREEN_COMMIT_PORT: u64 = crate::devices::port(crate::devices::SCREEN, 0x01);

const DRAW_NATIVES: &[&str] = &[
    "cls", "pset", "rect", "rectb", "rectmix", "dither", "line", "linew",
    "circ", "circb", "tri", "trib", "pal", "blit", "blitg", "blitr", "sprite", "save_png",
];

// System, Console, Screen, Controller, MIDI + abrase effect-dispatch ABI
// ports (0xE0/E1/E2: DISPATCH_ID/MODULE_ID, every effectful cart emits these).
pub const KNOWN_DEVICE_IDS: &[u8] = crate::devices::KNOWN_IDS;

fn known_device(id: u8) -> bool {
    KNOWN_DEVICE_IDS.contains(&id)
}

fn fold(a: Option<u64>, b: Option<u64>, f: impl Fn(u64, u64) -> u64) -> Option<u64> {
    match (a, b) { (Some(x), Some(y)) => Some(f(x, y)), _ => None }
}

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
            Chunk::Native(n) if n.name == want || n.name.strip_prefix("gfx_") == Some(want) => Some(i),
            _ => None,
        })
    };
    let screen_id = native_id("screen");
    let screen_off_id = native_id("screen_off");
    let commit_id = native_id("commit");
    let draw_ids: BTreeSet<usize> = module.functions.iter().enumerate().filter_map(|(i, c)| match c {
        Chunk::Native(n) if DRAW_NATIVES.contains(&n.name.strip_prefix("gfx_").unwrap_or(&n.name)) => Some(i),
        _ => None,
    }).collect();
    let update_fid = module.exports.iter().find(|e| e.name == "update").map(|e| e.fn_id as usize);

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
                OpCode::AddImm(d, s, imm) => reg_const[d.0 as usize] = reg_const[s.0 as usize].map(|v| v.wrapping_add(*imm as i64 as u64)),
                OpCode::SubImm(d, s, imm) => reg_const[d.0 as usize] = reg_const[s.0 as usize].map(|v| v.wrapping_sub(*imm as i64 as u64)),
                OpCode::Add(d, a, b) => reg_const[d.0 as usize] = fold(reg_const[a.0 as usize], reg_const[b.0 as usize], |x, y| x.wrapping_add(y)),
                OpCode::Sub(d, a, b) => reg_const[d.0 as usize] = fold(reg_const[a.0 as usize], reg_const[b.0 as usize], |x, y| x.wrapping_sub(y)),
                OpCode::Or(d, a, b) => reg_const[d.0 as usize] = fold(reg_const[a.0 as usize], reg_const[b.0 as usize], |x, y| x | y),
                OpCode::Shl(d, a, b) => reg_const[d.0 as usize] = fold(reg_const[a.0 as usize], reg_const[b.0 as usize], |x, y| x.wrapping_shl(y as u32)),
                OpCode::Jmp(_) | OpCode::Jz(_, _) | OpCode::Jnz(_, _) => reg_const = [None; 256],
                OpCode::CallReg(d, _) => reg_const[d.0 as usize] = None,
                OpCode::Call(d, fid) => {
                    let fid = *fid as usize;
                    facts[fidx].callees.push(fid);
                    if Some(fid) == screen_id {
                        facts[fidx].screen_sites.push(line_at(oi));
                        facts[fidx].opens_screen.set(line_at(oi));
                        seen_screen_here = true;
                    }
                    if Some(fid) == commit_id { facts[fidx].commits.set(line_at(oi)); }
                    if draw_ids.contains(&fid) { facts[fidx].draws.set(line_at(oi)); }
                    if Some(fid) == screen_off_id && seen_screen_here {
                        let mut w = PosaraLint::new("screen_order",
                            "screen_off() called after screen() in the same fn (must precede it)");
                        if let Some(l) = line_at(oi) { w = w.with_line(l); }
                        warns.push(w);
                    }
                    // callee gets its own window; only dest is clobbered.
                    reg_const[d.0 as usize] = None;
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

    // only the per-frame commit is gated on reachability from update().
    let reach_update = update_fid.map(|u| reachable(&[u], &facts)).unwrap_or_default();

    let any = |pick: &dyn Fn(&FnFacts) -> &Fact| -> Option<Option<usize>> {
        facts.iter().filter(|f| pick(f).hit).map(|f| pick(f).line).next()
    };
    let opens = any(&|f| &f.opens_screen);
    let draws = any(&|f| &f.draws);
    let commit_in_update = reach_update.iter().any(|&f| facts[f].commits.hit);

    let screen_sites: usize = facts.iter().map(|f| f.screen_sites.len()).sum();
    if screen_sites > 1 {
        let mut w = PosaraLint::new("screen_multi_call",
            format!("screen() called from {screen_sites} sites; it must be called exactly once"));
        if let Some(Some(l)) = opens { w = w.with_line(l); }
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

    if update_fid.is_some() && opens.is_some() && !commit_in_update {
        let mut w = PosaraLint::new("missing_commit_frame",
            "opens a screen but update() never commits it (device_in(0x2001,1)); window stays blank");
        if let Some(Some(l)) = opens { w = w.with_line(l); }
        warns.push(w);
    }
    if draws.is_some() && opens.is_none() {
        let mut w = PosaraLint::new("draw_without_screen",
            "draws but never opens a screen (screen()/device_in(0x2000,..)); framebuffer is 0x0, nothing shows");
        if let Some(Some(l)) = draws { w = w.with_line(l); }
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


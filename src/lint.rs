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

    let mut warns: Vec<PosaraLint> = Vec::new();
    let mut screen_sites = 0usize;
    let mut opens_screen = false;
    let mut draws = false;
    let mut commits = false;
    let mut bad_ports: BTreeSet<u64> = BTreeSet::new();

    for (fidx, chunk) in module.functions.iter().enumerate() {
        let Chunk::Bytecode(bc) = chunk else { continue };
        let mut reg_const: [Option<u64>; 256] = [None; 256];
        let mut seen_screen_here = false;
        for op in &bc.code {
            match op {
                OpCode::PushConst(r, idx) => reg_const[r.0 as usize] = bc.constants.get(*idx as usize).copied(),
                OpCode::Call(_, fid) => {
                    let fid = *fid as usize;
                    if Some(fid) == screen_id {
                        screen_sites += 1;
                        opens_screen = true;
                        seen_screen_here = true;
                        if Some(fidx) == update_fid {
                            warns.push(PosaraLint::new("screen_in_update",
                                "screen() called inside update(); call it once in start()"));
                        }
                    }
                    if draw_ids.contains(&fid) { draws = true; }
                    if Some(fid) == screen_off_id && seen_screen_here {
                        warns.push(PosaraLint::new("screen_order",
                            "screen_off() called after screen() in the same fn (must precede it)"));
                    }
                    reg_const = [None; 256];
                }
                OpCode::Dei(_, p) | OpCode::Deo(_, p) => {
                    if let Some(port) = reg_const[p.0 as usize] {
                        match port {
                            SCREEN_CONFIG_PORT => opens_screen = true,
                            SCREEN_COMMIT_PORT => commits = true,
                            _ => {}
                        }
                        let id = ((port >> 8) & 0xFF) as u8;
                        if port <= 0xFFFF && !known_device(id) {
                            bad_ports.insert(port);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    if screen_sites > 1 {
        warns.push(PosaraLint::new("screen_multi_call",
            format!("screen() called from {screen_sites} sites; it must be called exactly once")));
    }
    if update_fid.is_some() && opens_screen && !commits {
        warns.push(PosaraLint::new("missing_commit_frame",
            "opens a screen but never commits it (device_in(0x2001,1)); window stays blank"));
    }
    if draws && !opens_screen {
        warns.push(PosaraLint::new("draw_without_screen",
            "draws but never opens a screen (screen()/device_in(0x2000,..)); framebuffer is 0x0, nothing shows"));
    }
    for p in &bad_ports {
        warns.push(PosaraLint::new("device_port_out_of_range",
            format!("device port {:#06x} targets unknown device {:#04x} (typo?)", p, (p >> 8) & 0xFF)));
    }
    warns
}


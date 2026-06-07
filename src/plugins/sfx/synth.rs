use super::env::Adsr;
use super::fx::Fx;
use super::lfo::Lfo;
use super::seq::note_to_freq;

const MAX_PATCHES: usize = 16;
const MAX_VOICES: usize = 32;
const MAX_UNISON: usize = 7;

const FK_HI: u8 = 1;
const FK_BAND: u8 = 2;
const FK_LPG: u8 = 3;

const TGT_AMP: u8 = 0;
const TGT_CUTOFF: u8 = 1;
const TGT_PITCH: u8 = 2;

#[derive(Clone, Copy)]
struct OscCfg {
    wave: u8,
    mult: f32,
    level: f32,
}

impl Default for OscCfg {
    fn default() -> Self {
        Self { wave: 3, mult: 1.0, level: 0.0 }
    }
}

#[derive(Clone, Copy, Default)]
struct EnvCfg {
    target: u8,
    depth: f32,
    atk: f32,
    dec: f32,
    sus: f32,
    rel: f32,
}

#[derive(Clone, Copy)]
struct Patch {
    osc: [OscCfg; 2],
    fkind: u8,
    cutoff: f32,
    reso: f32,
    env: [EnvCfg; 2],
    lfo: Lfo,
    unison: u8,
    detune: f32,
    fx_kind: u8,
    fx_amt: f32,
    fx_param: f32,
}

impl Default for Patch {
    fn default() -> Self {
        let mut osc0 = OscCfg::default();
        osc0.level = 1.0;
        Self {
            osc: [osc0, OscCfg::default()],
            fkind: 0,
            cutoff: 18000.0,
            reso: 0.0,
            env: [
                EnvCfg { target: TGT_AMP, depth: 100.0, atk: 0.0, dec: 0.0, sus: 1.0, rel: 0.05 },
                // slot1 defaults to an inert cutoff mod (depth 0) — never silences a
                // patch that only configures slot0. Default-AMP here would zero amp.
                EnvCfg { target: TGT_CUTOFF, depth: 0.0, atk: 0.0, dec: 0.0, sus: 0.0, rel: 0.0 },
            ],
            lfo: Lfo::default(),
            unison: 1,
            detune: 0.0,
            fx_kind: 0,
            fx_amt: 0.0,
            fx_param: 0.0,
        }
    }
}

#[derive(Clone, Copy)]
struct Voice {
    cfg: Patch,
    pid: usize,
    note: u8,
    vel: f32,
    active: bool,
    gate: bool,
    gate_samples: u32,
    age: u64,
    base_freq: f32,
    env: [Adsr; 2],
    lfo: Lfo,
    fx: Fx,
    phase: [[f32; 2]; MAX_UNISON],
    uratio: [f32; MAX_UNISON],
    ucount: usize,
    lfsr: u32,
    flp: f32,
    fbp: f32,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            cfg: Patch::default(),
            pid: 0,
            note: 0,
            vel: 0.0,
            active: false,
            gate: false,
            gate_samples: 0,
            age: 0,
            base_freq: 0.0,
            env: [Adsr::default(); 2],
            lfo: Lfo::default(),
            fx: Fx::default(),
            phase: [[0.0; 2]; MAX_UNISON],
            uratio: [1.0; MAX_UNISON],
            ucount: 1,
            lfsr: 0x1234abcd,
            flp: 0.0,
            fbp: 0.0,
        }
    }
}

fn wave_sample(wave: u8, phase: f32, lfsr: &mut u32) -> f32 {
    if wave == 4 {
        let mut x = if *lfsr == 0 { 0x1234abcd } else { *lfsr };
        x ^= x << 13; x ^= x >> 17; x ^= x << 5;
        *lfsr = x;
        return ((x & 0xFF) as f32 / 128.0) - 1.0;
    }
    match wave {
        1 => (phase * std::f32::consts::TAU).sin(),
        2 => if phase < 0.5 { 4.0 * phase - 1.0 } else { 3.0 - 4.0 * phase },
        3 => 2.0 * phase - 1.0,
        _ => if phase < 0.5 { 1.0 } else { -1.0 },
    }
}

impl Voice {
    fn start(&mut self, p: &Patch, pid: usize, note: u8, vel: f32, gate_samples: u32, age: u64) {
        self.cfg = *p;
        self.pid = pid;
        self.note = note;
        self.vel = vel.clamp(0.0, 1.0);
        self.base_freq = note_to_freq(note);
        self.active = true;
        self.gate = true;
        self.gate_samples = gate_samples;
        self.age = age;
        for i in 0..2 {
            self.env[i].set(p.env[i].atk, p.env[i].dec, p.env[i].sus, p.env[i].rel);
            self.env[i].gate_on();
        }
        self.lfo = p.lfo;
        self.fx = Fx::default();
        self.fx.set(p.fx_kind, p.fx_amt, p.fx_param);
        let c = (p.unison.max(1) as usize).min(MAX_UNISON);
        self.ucount = c;
        for u in 0..c {
            let spread = if c == 1 { 0.0 } else { (u as f32 / (c as f32 - 1.0)) * 2.0 - 1.0 };
            self.uratio[u] = (2.0f32).powf(spread * p.detune / 1200.0);
            self.phase[u] = [(u as f32 * 0.137) % 1.0, (u as f32 * 0.331) % 1.0];
        }
        self.lfsr = 0x1234abcd ^ note as u32 ^ (age as u32).wrapping_mul(2654435761);
        self.flp = 0.0;
        self.fbp = 0.0;
    }

    fn note_off(&mut self) {
        self.gate = false;
        for e in self.env.iter_mut() { e.gate_off(); }
    }

    fn filter(&mut self, x: f32, cutoff: f32, reso: f32, sr: f32) -> f32 {
        // Chamberlin SVF: stable only for fc well below sr/6 — clamp to keep f < 1,
        // else it blows up to inf/NaN (silent output that also dodges the meter).
        let fc = cutoff.clamp(20.0, sr * 0.16);
        let f = 2.0 * (std::f32::consts::PI * fc / sr).sin();
        let damp = (1.0 - (reso / 100.0) * 0.9).clamp(0.1, 1.0);
        self.flp += f * self.fbp;
        let hp = x - self.flp - damp * self.fbp;
        self.fbp += f * hp;
        if !self.flp.is_finite() || !self.fbp.is_finite() { self.flp = 0.0; self.fbp = 0.0; }
        match self.cfg.fkind {
            FK_HI => hp,
            FK_BAND => self.fbp,
            _ => self.flp,
        }
    }

    fn tick(&mut self, sr: f32) -> f32 {
        if !self.active { return 0.0; }
        if self.gate_samples == 1 { self.note_off(); }
        if self.gate_samples > 0 { self.gate_samples -= 1; }

        let dt = 1.0 / sr;
        let mut amp = 1.0f32;
        let mut have_amp = false;
        let mut cut_oct = 0.0f32;
        let mut pitch_semi = 0.0f32;
        let mut any = false;
        let mut amp_env = 1.0f32;
        for i in 0..2 {
            let lv = self.env[i].tick(dt);
            if self.env[i].active() { any = true; }
            let ec = self.cfg.env[i];
            match ec.target {
                TGT_AMP => { amp *= lv; have_amp = true; amp_env = lv; }
                TGT_CUTOFF => cut_oct += (ec.depth / 100.0) * 6.0 * lv,
                TGT_PITCH => pitch_semi += ec.depth * lv,
                _ => {}
            }
        }
        if !have_amp { amp = 1.0; }
        if !any && !self.gate { self.active = false; return 0.0; }

        let m = if self.lfo.enabled() { self.lfo.tick(sr) } else { 0.0 };
        let (mut p_mul, mut cut_mul, mut amp_lfo) = (1.0f32, 1.0f32, 1.0f32);
        match self.lfo.target {
            0 => p_mul = (2.0f32).powf(m),
            1 => amp_lfo = (1.0 + m).max(0.0),
            2 => cut_mul = (2.0f32).powf(m * 2.0),
            _ => {}
        }

        let freq = self.base_freq * (2.0f32).powf(pitch_semi / 12.0) * p_mul;
        let mut s = 0.0f32;
        for u in 0..self.ucount {
            let uf = freq * self.uratio[u];
            for o in 0..2 {
                let oc = self.cfg.osc[o];
                if oc.level <= 0.0 { continue; }
                let of = uf * oc.mult;
                self.phase[u][o] = (self.phase[u][o] + of / sr) % 1.0;
                s += wave_sample(oc.wave, self.phase[u][o], &mut self.lfsr) * oc.level;
            }
        }
        s /= self.ucount as f32;

        let cutoff = if self.cfg.fkind == FK_LPG {
            self.cfg.cutoff * (0.05 + 0.95 * amp_env)
        } else {
            self.cfg.cutoff * (2.0f32).powf(cut_oct) * cut_mul
        };
        s = self.filter(s, cutoff, self.cfg.reso, sr);
        s *= amp * self.vel * amp_lfo;
        self.fx.process(s, sr)
    }
}

pub struct Synth {
    patches: [Patch; MAX_PATCHES],
    voices: [Voice; MAX_VOICES],
    pool: usize,
    age: u64,
    pid_peak: [f32; 4],
}

impl Synth {
    pub fn new() -> Self {
        Self {
            patches: [Patch::default(); MAX_PATCHES],
            voices: [Voice::default(); MAX_VOICES],
            pool: 8,
            age: 0,
            pid_peak: [0.0; 4],
        }
    }

    fn patch(&mut self, pid: usize) -> Option<&mut Patch> {
        self.patches.get_mut(pid)
    }

    pub fn osc(&mut self, pid: usize, idx: usize, wave: u8, semi: i64, fine: f32, level: f32) {
        if let Some(p) = self.patch(pid) {
            if idx < 2 {
                p.osc[idx] = OscCfg {
                    wave,
                    mult: (2.0f32).powf((semi as f32 + fine / 100.0) / 12.0),
                    level: level.max(0.0),
                };
            }
        }
    }

    pub fn filter(&mut self, pid: usize, kind: u8, cutoff: f32, reso: f32) {
        if let Some(p) = self.patch(pid) {
            p.fkind = kind;
            p.cutoff = cutoff.max(20.0);
            p.reso = reso.clamp(0.0, 100.0);
        }
    }

    pub fn env(&mut self, pid: usize, slot: usize, target: u8, depth: f32, atk: f32, dec: f32, sus: f32, rel: f32) {
        if let Some(p) = self.patch(pid) {
            if slot < 2 {
                p.env[slot] = EnvCfg { target, depth, atk, dec, sus: sus.clamp(0.0, 1.0), rel };
            }
        }
    }

    pub fn lfo(&mut self, pid: usize, target: u8, rate: f32, depth: f32) {
        if let Some(p) = self.patch(pid) {
            p.lfo.set(target, 0, rate, depth);
        }
    }

    pub fn unison(&mut self, pid: usize, count: u8, detune: f32) {
        if let Some(p) = self.patch(pid) {
            p.unison = count.clamp(1, MAX_UNISON as u8);
            p.detune = detune;
        }
    }

    pub fn fx(&mut self, pid: usize, kind: u8, amt: f32, param: f32) {
        if let Some(p) = self.patch(pid) {
            p.fx_kind = kind;
            p.fx_amt = amt;
            p.fx_param = param;
        }
    }

    pub fn set_voices(&mut self, n: usize) {
        self.pool = n.clamp(1, MAX_VOICES);
    }

    fn alloc(&self) -> usize {
        for i in 0..self.pool {
            if !self.voices[i].active { return i; }
        }
        let mut best = 0;
        let mut best_age = u64::MAX;
        for i in 0..self.pool {
            if self.voices[i].age < best_age { best_age = self.voices[i].age; best = i; }
        }
        best
    }

    pub fn note_on(&mut self, pid: usize, note: u8, vel: f32, gate_samples: u32) {
        if pid >= MAX_PATCHES { return; }
        self.age += 1;
        let slot = self.alloc();
        let p = self.patches[pid];
        self.voices[slot].start(&p, pid, note, vel, gate_samples, self.age);
    }

    pub fn note_off(&mut self, pid: usize, note: u8) {
        for i in 0..self.pool {
            let v = &mut self.voices[i];
            if v.active && v.gate && v.pid == pid && v.note == note { v.note_off(); }
        }
    }

    pub fn stop(&mut self, pid: usize) {
        for v in self.voices.iter_mut() {
            if v.active && v.pid == pid { v.note_off(); }
        }
    }

    pub fn panic(&mut self) {
        for v in self.voices.iter_mut() { v.active = false; }
    }

    pub fn tick(&mut self, sr: f32) -> f32 {
        let mut total = 0.0f32;
        let mut psum = [0.0f32; 4];
        for i in 0..self.pool {
            let pid = self.voices[i].pid;
            let s = self.voices[i].tick(sr);
            total += s;
            if pid < 4 { psum[pid] += s; }
        }
        for p in 0..4 {
            if psum[p].abs() > self.pid_peak[p] { self.pid_peak[p] = psum[p].abs(); }
        }
        total
    }

    pub fn take_pid_peaks(&mut self) -> [f32; 4] {
        let p = self.pid_peak;
        self.pid_peak = [0.0; 4];
        p
    }

    pub fn pid_voices(&self) -> [u32; 4] {
        let mut v = [0u32; 4];
        for i in 0..self.pool {
            let voice = &self.voices[i];
            if voice.active && voice.pid < 4 { v[voice.pid] += 1; }
        }
        v
    }
}

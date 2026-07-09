use crate::sfx::env::Adsr;
use crate::sfx::fx::Fx;
use crate::sfx::lfo::{Lfo, LFO_AMP, LFO_CUTOFF, LFO_PITCH};

pub const WAVE_SQUARE: u8 = 0;
pub const WAVE_SINE: u8 = 1;
pub const WAVE_TRIANGLE: u8 = 2;
pub const WAVE_SAW: u8 = 3;
pub const WAVE_NOISE: u8 = 4;

#[derive(Clone, Copy, Default)]
pub struct Voice {
    pub wave: u8,
    pub pan_l: f32,
    pub pan_r: f32,
    pub env: Adsr,
    pub lfo: Lfo,
    pub fx: Fx,
    freq: f32,
    vel: f32,
    phase: f32,
    lfsr: u32,
    gate_samples: u32,
}

impl Voice {
    pub fn new() -> Self {
        Self { pan_l: 1.0, pan_r: 1.0, env: Adsr::default(), ..Default::default() }
    }

    pub fn set_inst(&mut self, wave: u8, atk: f32, dec: f32, sus: f32, rel: f32) {
        self.wave = wave;
        self.env.set(atk, dec, sus, rel);
    }

    pub fn set_pan(&mut self, l: f32, r: f32) {
        self.pan_l = l.clamp(0.0, 1.0);
        self.pan_r = r.clamp(0.0, 1.0);
    }

    pub fn play(&mut self, freq: f32, vel: f32, gate_samples: u32) {
        self.freq = freq;
        self.vel = vel.clamp(0.0, 1.0);
        self.gate_samples = gate_samples;
        if self.wave == WAVE_NOISE { self.lfsr = self.lfsr.wrapping_add(0xabcd1234).max(1); }
        self.env.gate_on();
    }

    pub fn off(&mut self) {
        self.gate_samples = 0;
        self.env.gate_off();
    }

    pub fn active(&self) -> bool {
        self.env.active()
    }

    fn osc(&mut self, freq: f32, sr: f32) -> f32 {
        if self.wave == WAVE_NOISE {
            let mut x = if self.lfsr == 0 { 0x1234abcd } else { self.lfsr };
            x ^= x << 13; x ^= x >> 17; x ^= x << 5;
            self.lfsr = x;
            return ((x & 0xFF) as f32 / 128.0) - 1.0;
        }
        self.phase = (self.phase + freq / sr) % 1.0;
        let p = self.phase;
        match self.wave {
            WAVE_SINE => (p * std::f32::consts::TAU).sin(),
            WAVE_TRIANGLE => if p < 0.5 { 4.0 * p - 1.0 } else { 3.0 - 4.0 * p },
            WAVE_SAW => 2.0 * p - 1.0,
            _ => if p < 0.5 { 1.0 } else { -1.0 },
        }
    }

    pub fn tick(&mut self, sr: f32) -> (f32, f32) {
        if !self.env.active() { return (0.0, 0.0); }
        if self.gate_samples == 1 { self.env.gate_off(); }
        if self.gate_samples > 0 { self.gate_samples -= 1; }

        let m = if self.lfo.enabled() { self.lfo.tick(sr) } else { 0.0 };
        let freq = if self.lfo.target == LFO_PITCH { self.freq * (2.0f32).powf(m) } else { self.freq };
        let mut s = self.osc(freq, sr);

        let amp = self.env.tick(1.0 / sr) * self.vel;
        s *= amp;
        if self.lfo.target == LFO_AMP { s *= (1.0 + m).max(0.0); }

        if self.lfo.target == LFO_CUTOFF && self.fx.kind != 0 {
            let base = self.fx.param;
            self.fx.param = base * (2.0f32).powf(m * 2.0);
            s = self.fx.process(s, sr);
            self.fx.param = base;
        } else {
            s = self.fx.process(s, sr);
        }
        (s * self.pan_l, s * self.pan_r)
    }
}

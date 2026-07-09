pub const LFO_PITCH: u8 = 0;
pub const LFO_AMP: u8 = 1;
pub const LFO_CUTOFF: u8 = 2;

#[derive(Clone, Copy, Default)]
pub struct Lfo {
    pub target: u8,
    pub wave: u8,
    pub rate: f32,
    pub depth: f32,
    phase: f32,
}

impl Lfo {
    pub fn set(&mut self, target: u8, wave: u8, rate: f32, depth: f32) {
        self.target = target; self.wave = wave; self.rate = rate;
        self.depth = depth.clamp(0.0, 1.0);
    }

    pub fn enabled(&self) -> bool {
        self.depth > 0.0 && self.rate > 0.0
    }

    pub fn tick(&mut self, sr: f32) -> f32 {
        self.phase = (self.phase + self.rate / sr) % 1.0;
        let p = self.phase;
        let s = match self.wave {
            1 => if p < 0.5 { 4.0 * p - 1.0 } else { 3.0 - 4.0 * p },
            2 => 2.0 * p - 1.0,
            3 => if p < 0.5 { 1.0 } else { -1.0 },
            _ => (p * std::f32::consts::TAU).sin(),
        };
        s * self.depth
    }
}

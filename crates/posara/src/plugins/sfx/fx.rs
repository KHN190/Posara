pub const FX_BITCRUSH: u8 = 1;
pub const FX_DRIVE: u8 = 2;
pub const FX_LOPASS: u8 = 3;
pub const FX_HIPASS: u8 = 4;
pub const FX_RING: u8 = 5;

#[derive(Clone, Copy, Default)]
pub struct Fx {
    pub kind: u8,
    pub amt: f32,
    pub param: f32,
    hold: f32,
    hold_n: u32,
    lp: f32,
    ring_phase: f32,
}

impl Fx {
    pub fn set(&mut self, kind: u8, amt: f32, param: f32) {
        self.kind = kind; self.amt = amt.clamp(0.0, 1.0); self.param = param;
    }

    pub fn process(&mut self, x: f32, sr: f32) -> f32 {
        match self.kind {
            FX_BITCRUSH => {
                let levels = (2.0f32).powf(1.0 + (1.0 - self.amt) * 11.0); // amt↑ → fewer bits
                let q = (x * 0.5 + 0.5) * levels;
                let crushed = (q.round() / levels) * 2.0 - 1.0;
                let decim = 1 + (self.amt * 16.0) as u32; // amt↑ → coarser sample-hold
                if self.hold_n == 0 { self.hold = crushed; self.hold_n = decim; }
                self.hold_n -= 1;
                self.hold
            }
            FX_DRIVE => {
                let g = 1.0 + self.amt * 24.0;
                (x * g).tanh()
            }
            FX_LOPASS => {
                let cut = self.param.max(20.0);
                let a = (1.0 - (-std::f32::consts::TAU * cut / sr).exp()).clamp(0.0, 1.0);
                self.lp += a * (x - self.lp);
                self.lp
            }
            FX_HIPASS => {
                let cut = self.param.max(20.0);
                let a = (1.0 - (-std::f32::consts::TAU * cut / sr).exp()).clamp(0.0, 1.0);
                self.lp += a * (x - self.lp);
                x - self.lp
            }
            FX_RING => {
                self.ring_phase = (self.ring_phase + self.param.max(0.0) / sr) % 1.0;
                let m = (self.ring_phase * std::f32::consts::TAU).sin();
                x * (1.0 - self.amt + self.amt * m)
            }
            _ => x,
        }
    }
}

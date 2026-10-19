pub fn to_i16(s: f32) -> i16 {
    (s.clamp(-1.0, 1.0) * 32767.0) as i16
}

pub fn to_mono_i16(stereo: &[f32], out: &mut [i16]) {
    for (o, lr) in out.iter_mut().zip(stereo.chunks_exact(2)) {
        *o = to_i16((lr[0] + lr[1]) * 0.5);
    }
}

// Headphone width: right lags left by a fixed number of samples (Haas).
pub struct Width {
    ring: alloc::vec::Vec<f32>,
    at: usize,
    on: bool,
}

impl Width {
    pub fn new(lag: usize) -> Self { Self { ring: alloc::vec![0.0; lag.max(1)], at: 0, on: false } }

    pub fn set(&mut self, on: bool) { self.on = on; }

    pub fn next(&mut self, x: f32) -> (f32, f32) {
        let late = core::mem::replace(&mut self.ring[self.at], x);
        self.at = (self.at + 1) % self.ring.len();
        (x, if self.on { late } else { x })
    }
}

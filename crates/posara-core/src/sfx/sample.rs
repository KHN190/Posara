// 1-bit sample player: a delta-sigma bitstream (MSB-first packed) read at
// `rate` Hz, each bit → ±1, through a 1-pole lowpass that reconstructs the
// low-freq signal (the noise-shaped quantization error lives up high and is
// filtered out). The playback-side half of mp32sample.
use alloc::vec::Vec;
use crate::math;

pub trait Pos: Default {
    fn bit(&self) -> usize;
    fn advance(&mut self, step: f32);
}

#[cfg(not(all(target_os = "none", target_pointer_width = "32")))]
impl Pos for f64 {
    fn bit(&self) -> usize { *self as usize }
    fn advance(&mut self, step: f32) { *self += step as f64; }
}

#[derive(Default)]
pub struct Fixed { whole: u32, fraction: u32 }

impl Pos for Fixed {
    fn bit(&self) -> usize { if self.whole == u32::MAX { usize::MAX } else { self.whole as usize } }
    fn advance(&mut self, step: f32) {
        let whole = step as u32;
        let fraction = ((step - whole as f32) * 4294967296.0) as u32;
        let (fraction, carry) = self.fraction.overflowing_add(fraction);
        self.fraction = fraction;
        self.whole = self.whole.saturating_add(whole).saturating_add(carry as u32);
    }
}

#[derive(Default)]
pub struct SamplePlayer<P: Pos = Fixed> {
    bytes: Vec<u8>,
    total_bits: usize,
    rate: f32,
    vol: f32,
    pos: P,
    lp: f32,
    playing: bool,
    cached_sr: f32,
    step: f32,
    alpha: f32,
}

impl<P: Pos> SamplePlayer<P> {
    pub fn play(&mut self, bytes: Vec<u8>, rate: f32, vol: f32) -> Vec<u8> {
        self.total_bits = bytes.len().saturating_mul(8);
        let old = core::mem::replace(&mut self.bytes, bytes);
        self.rate = rate.max(1.0);
        self.vol = vol.clamp(0.0, 1.0);
        self.pos = P::default();
        self.lp = 0.0;
        self.cached_sr = 0.0;
        self.playing = self.total_bits > 0;
        old
    }

    pub fn stop(&mut self) {
        self.playing = false;
    }

    pub fn tick(&mut self, sr: f32) -> f32 {
        if !self.playing {
            return 0.0;
        }
        let bi = self.pos.bit();
        if bi >= self.total_bits {
            self.playing = false;
            return 0.0;
        }
        if self.cached_sr != sr {
            self.cached_sr = sr;
            self.step = self.rate / sr;
            let cut = self.rate * 0.45;
            self.alpha = (1.0 - math::exp(-core::f32::consts::TAU * cut / sr)).clamp(0.0, 1.0);
        }
        self.pos.advance(self.step);
        let bit = (self.bytes[bi / 8] >> (7 - (bi & 7))) & 1;
        let raw = if bit == 1 { 1.0 } else { -1.0 };
        self.lp += self.alpha * (raw - self.lp);
        self.lp * self.vol
    }
}

//! PSM1: magic, little-endian u32 sample rate and frame count, mono i16 PCM.
#[derive(Clone, Copy)]
pub struct Clip<'a> {
    bytes: &'a [u8],
    pub rate: u32,
    pub frames: usize,
}
impl<'a> Clip<'a> {
    pub fn parse(bytes: &'a [u8]) -> Option<Self> {
        if bytes.len() < 12 || &bytes[..4] != b"PSM1" {
            return None;
        }
        let rate = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
        let frames = u32::from_le_bytes(bytes[8..12].try_into().ok()?) as usize;
        if !(8000..=96000).contains(&rate)
            || frames == 0
            || frames.checked_mul(2)?.checked_add(12)? != bytes.len()
        {
            return None;
        }
        Some(Self {
            bytes,
            rate,
            frames,
        })
    }
    #[inline(always)]
    pub fn sample(self, index: usize) -> f32 {
        if index >= self.frames {
            return 0.0;
        }
        let i = 12 + index * 2;
        i16::from_le_bytes([self.bytes[i], self.bytes[i + 1]]) as f32 / 32768.0
    }
}
#[derive(Default)]
pub struct Player<'a> {
    clip: Option<Clip<'a>>,
    phase: u64,
    step: u64,
    gain: f32,
}
impl<'a> Player<'a> {
    pub fn play(&mut self, clip: Clip<'a>, output_rate: u32, gain: f32) {
        self.step = ((clip.rate as u64) << 32) / output_rate.max(1) as u64;
        self.clip = Some(clip);
        self.phase = 0;
        self.gain = gain.clamp(0.0, 1.0);
    }
    #[inline]
    pub fn stop(&mut self) {
        self.clip = None;
    }
    #[inline]
    pub fn active(&self) -> bool {
        self.clip.is_some()
    }
    #[inline(always)]
    pub fn tick(&mut self) -> f32 {
        let Some(clip) = self.clip else {
            return 0.0;
        };
        let i = (self.phase >> 32) as usize;
        if i >= clip.frames {
            self.clip = None;
            return 0.0;
        }
        let t = (self.phase as u32) as f32 * (1.0 / 4294967296.0);
        self.phase += self.step;
        let a = clip.sample(i);
        (a + (clip.sample((i + 1).min(clip.frames - 1)) - a) * t) * self.gain
    }
}

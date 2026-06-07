// 1-bit sample player: a delta-sigma bitstream (MSB-first packed) read at
// `rate` Hz, each bit → ±1, through a 1-pole lowpass that reconstructs the
// low-freq signal (the noise-shaped quantization error lives up high and is
// filtered out). The playback-side half of mp32sample.

#[derive(Default)]
pub struct SamplePlayer {
    bytes: Vec<u8>,
    total_bits: usize,
    rate: f32,
    vol: f32,
    pos: f64,
    lp: f32,
    playing: bool,
}

impl SamplePlayer {
    pub fn play(&mut self, bytes: Vec<u8>, rate: f32, vol: f32) {
        self.total_bits = bytes.len() * 8;
        self.bytes = bytes;
        self.rate = rate.max(1.0);
        self.vol = vol.clamp(0.0, 1.0);
        self.pos = 0.0;
        self.lp = 0.0;
        self.playing = self.total_bits > 0;
    }

    pub fn stop(&mut self) {
        self.playing = false;
    }

    pub fn tick(&mut self, sr: f32) -> f32 {
        if !self.playing {
            return 0.0;
        }
        let bi = self.pos as usize;
        if bi >= self.total_bits {
            self.playing = false;
            return 0.0;
        }
        self.pos += (self.rate / sr) as f64;
        let bit = (self.bytes[bi / 8] >> (7 - (bi & 7))) & 1;
        let raw = if bit == 1 { 1.0 } else { -1.0 };
        let cut = self.rate * 0.45;
        let a = (1.0 - (-std::f32::consts::TAU * cut / sr).exp()).clamp(0.0, 1.0);
        self.lp += a * (raw - self.lp);
        self.lp * self.vol
    }
}

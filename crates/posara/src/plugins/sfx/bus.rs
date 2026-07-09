// Master-bus time effects: a stereo delay and a compact Schroeder reverb.
// Both run after every voice is summed, so their tails outlive the notes.

pub struct Delay {
    bl: Vec<f32>,
    br: Vec<f32>,
    idx: usize,
    len: usize,
    fb: f32,
    mix: f32,
}

impl Default for Delay {
    fn default() -> Self {
        Self { bl: vec![0.0; 1], br: vec![0.0; 1], idx: 0, len: 0, fb: 0.0, mix: 0.0 }
    }
}

impl Delay {
    // time_ms 0 disables. fb/mix 0..1.
    pub fn set(&mut self, time_ms: u32, fb: f32, mix: f32, sr: f32) {
        let len = ((time_ms as f32) * sr / 1000.0) as usize;
        if len != self.len {
            self.bl = vec![0.0; len.max(1)];
            self.br = vec![0.0; len.max(1)];
            self.idx = 0;
        }
        self.len = len;
        self.fb = fb.clamp(0.0, 0.97);
        self.mix = mix.clamp(0.0, 1.0);
    }

    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        if self.len == 0 || self.mix <= 0.0 { return (l, r); }
        let wl = self.bl[self.idx];
        let wr = self.br[self.idx];
        self.bl[self.idx] = l + wl * self.fb;
        self.br[self.idx] = r + wr * self.fb;
        self.idx = (self.idx + 1) % self.len;
        (l + wl * self.mix, r + wr * self.mix)
    }
}

struct Comb { buf: Vec<f32>, idx: usize, store: f32, fb: f32, damp1: f32, damp2: f32 }

impl Comb {
    fn new(len: usize) -> Self {
        Self { buf: vec![0.0; len.max(1)], idx: 0, store: 0.0, fb: 0.0, damp1: 0.0, damp2: 1.0 }
    }
    fn process(&mut self, x: f32) -> f32 {
        let out = self.buf[self.idx];
        self.store = out * self.damp2 + self.store * self.damp1;
        self.buf[self.idx] = x + self.store * self.fb;
        self.idx = (self.idx + 1) % self.buf.len();
        out
    }
}

struct Allpass { buf: Vec<f32>, idx: usize }

impl Allpass {
    fn new(len: usize) -> Self {
        Self { buf: vec![0.0; len.max(1)], idx: 0 }
    }
    fn process(&mut self, x: f32) -> f32 {
        let buf = self.buf[self.idx];
        let out = buf - x;
        self.buf[self.idx] = x + buf * 0.5;
        self.idx = (self.idx + 1) % self.buf.len();
        out
    }
}

// Freeverb-lite: 4 combs in parallel into 2 allpass in series, mono.
pub struct Reverb {
    combs: Vec<Comb>,
    aps: Vec<Allpass>,
    mix: f32,
}

impl Reverb {
    pub fn new(sr: f32) -> Self {
        let s = sr / 44100.0;
        let ct = [1116, 1188, 1277, 1356];
        let at = [556, 441];
        Self {
            combs: ct.iter().map(|&n| Comb::new((n as f32 * s) as usize)).collect(),
            aps: at.iter().map(|&n| Allpass::new((n as f32 * s) as usize)).collect(),
            mix: 0.0,
        }
    }

    // room/damp/mix 0..1.
    pub fn set(&mut self, room: f32, damp: f32, mix: f32) {
        let fb = 0.7 + room.clamp(0.0, 1.0) * 0.28;
        let d = damp.clamp(0.0, 1.0);
        for c in self.combs.iter_mut() {
            c.fb = fb;
            c.damp1 = d;
            c.damp2 = 1.0 - d;
        }
        self.mix = mix.clamp(0.0, 1.0);
    }

    // returns wet signal already scaled by mix (caller adds to dry).
    pub fn process(&mut self, x: f32) -> f32 {
        if self.mix <= 0.0 { return 0.0; }
        let mut s = 0.0;
        for c in self.combs.iter_mut() { s += c.process(x); }
        s *= 0.25;
        for a in self.aps.iter_mut() { s = a.process(s); }
        s * self.mix
    }
}

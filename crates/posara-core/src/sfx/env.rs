#[derive(Default, Clone, Copy, PartialEq)]
enum Stage {
    #[default]
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Clone, Copy)]
pub struct Adsr {
    atk: f32,
    dec: f32,
    pub sus: f32,
    rel: f32,
    inverse: [f32; 3],
    stage: Stage,
    t: f32,
    level: f32,
    rel_from: f32,
}

impl Default for Adsr {
    fn default() -> Self {
        Self { atk: 0.0, dec: 0.0, sus: 1.0, rel: 0.0, inverse: [0.0; 3], stage: Stage::Idle, t: 0.0, level: 0.0, rel_from: 0.0 }
    }
}

impl Adsr {
    pub fn set(&mut self, atk: f32, dec: f32, sus: f32, rel: f32) {
        self.atk = atk; self.dec = dec; self.sus = sus.clamp(0.0, 1.0); self.rel = rel;
        self.inverse = [atk, dec, rel].map(|duration| if duration > 0.0 { 1.0 / duration } else { 0.0 });
    }

    pub fn gate_on(&mut self) {
        self.stage = Stage::Attack;
        self.t = 0.0;
    }

    pub fn gate_off(&mut self) {
        if self.stage != Stage::Idle && self.stage != Stage::Release {
            self.rel_from = self.level;
            self.stage = Stage::Release;
            self.t = 0.0;
        }
    }

    pub fn active(&self) -> bool {
        self.stage != Stage::Idle
    }

    #[inline]
    fn progress(&self, duration: f32, inverse: f32) -> f32 {
        #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
        {
            let _ = duration;
            if self.t == 0.0 { 0.0 } else { (self.t * inverse).min(1.0) }
        }
        #[cfg(not(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32"))))]
        {
            let _ = inverse;
            (self.t / duration).min(1.0)
        }
    }

    pub fn tick(&mut self, dt: f32) -> f32 {
        match self.stage {
            Stage::Idle => self.level = 0.0,
            Stage::Attack => {
                self.level = if self.atk <= 0.0 { 1.0 } else { self.progress(self.atk, self.inverse[0]) };
                self.t += dt;
                if self.t >= self.atk { self.stage = Stage::Decay; self.t = 0.0; }
            }
            Stage::Decay => {
                self.level = if self.dec <= 0.0 { self.sus } else { 1.0 - (1.0 - self.sus) * self.progress(self.dec, self.inverse[1]) };
                self.t += dt;
                if self.t >= self.dec { self.stage = Stage::Sustain; }
            }
            Stage::Sustain => self.level = self.sus,
            Stage::Release => {
                self.level = if self.rel <= 0.0 { 0.0 } else { self.rel_from * (1.0 - self.progress(self.rel, self.inverse[2])) };
                self.t += dt;
                if self.t >= self.rel { self.stage = Stage::Idle; self.level = 0.0; }
            }
        }
        self.level
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    mod reference {
        #[derive(Default, Clone, Copy, PartialEq)]
        enum Stage {
            #[default]
            Idle,
            Attack,
            Decay,
            Sustain,
            Release,
        }

        #[derive(Clone, Copy)]
        pub struct Adsr {
            pub atk: f32,
            pub dec: f32,
            pub sus: f32,
            pub rel: f32,
            stage: Stage,
            t: f32,
            level: f32,
            rel_from: f32,
        }

        impl Default for Adsr {
            fn default() -> Self {
                Self { atk: 0.0, dec: 0.0, sus: 1.0, rel: 0.0, stage: Stage::Idle, t: 0.0, level: 0.0, rel_from: 0.0 }
            }
        }

        impl Adsr {
            pub fn set(&mut self, atk: f32, dec: f32, sus: f32, rel: f32) {
                self.atk = atk; self.dec = dec; self.sus = sus.clamp(0.0, 1.0); self.rel = rel;
            }

            pub fn gate_on(&mut self) {
                self.stage = Stage::Attack;
                self.t = 0.0;
            }

            pub fn gate_off(&mut self) {
                if self.stage != Stage::Idle && self.stage != Stage::Release {
                    self.rel_from = self.level;
                    self.stage = Stage::Release;
                    self.t = 0.0;
                }
            }

            pub fn active(&self) -> bool {
                self.stage != Stage::Idle
            }

            pub fn tick(&mut self, dt: f32) -> f32 {
                match self.stage {
                    Stage::Idle => self.level = 0.0,
                    Stage::Attack => {
                        self.level = if self.atk <= 0.0 { 1.0 } else { (self.t / self.atk).min(1.0) };
                        self.t += dt;
                        if self.t >= self.atk { self.stage = Stage::Decay; self.t = 0.0; }
                    }
                    Stage::Decay => {
                        self.level = if self.dec <= 0.0 { self.sus } else { 1.0 - (1.0 - self.sus) * (self.t / self.dec).min(1.0) };
                        self.t += dt;
                        if self.t >= self.dec { self.stage = Stage::Sustain; }
                    }
                    Stage::Sustain => self.level = self.sus,
                    Stage::Release => {
                        self.level = if self.rel <= 0.0 { 0.0 } else { self.rel_from * (1.0 - (self.t / self.rel).min(1.0)) };
                        self.t += dt;
                        if self.t >= self.rel { self.stage = Stage::Idle; self.level = 0.0; }
                    }
                }
                self.level
            }
        }
        impl Adsr {
            pub fn state(&self) -> (u8, u32) { (self.stage as u8, self.t.to_bits()) }
        }
    }

    #[test]
    fn reciprocal_envelope_preserves_timing_and_bounds_error() {
        let mut worst = 0.0_f32;
        for sr in [8000.0, 22050.0, 44100.0, 48000.0, 96000.0] {
            for duration in [0.0, f32::from_bits(1), 0.00001, 0.01, 0.55, 2.0] {
                for sustain in [0.0, 0.4, 1.0] {
                    let mut actual = Adsr::default();
                    let mut expected = reference::Adsr::default();
                    for n in 0..24000 {
                        if n == 0 || n == 12000 {
                            actual.set(duration, duration, sustain, duration);
                            expected.set(duration, duration, sustain, duration);
                            actual.gate_on(); expected.gate_on();
                        }
                        if n == 4500 || n == 17000 { actual.gate_off(); expected.gate_off(); }
                        let a = actual.tick(1.0 / sr);
                        let b = expected.tick(1.0 / sr);
                        worst = worst.max((a-b).abs());
                        assert!(a.is_finite() && (a-b).abs() <= 4.0 * f32::EPSILON);
                        assert_eq!(actual.active(), expected.active());
                        assert_eq!((actual.stage as u8, actual.t.to_bits()), expected.state());
                        if b == 0.0 { assert_eq!(a, 0.0); }
                    }
                }
            }
        }
        assert!(worst <= 4.0 * f32::EPSILON);
    }
}

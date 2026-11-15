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
    exponential: bool,
    stage: Stage,
    t: f32,
    level: f32,
    rel_from: f32,
}

impl Default for Adsr {
    fn default() -> Self {
        Self { atk: 0.0, dec: 0.0, sus: 1.0, rel: 0.0, inverse: [0.0; 3], exponential:false, stage: Stage::Idle, t: 0.0, level: 0.0, rel_from: 0.0 }
    }
}

impl Adsr {
    pub fn set(&mut self, atk: f32, dec: f32, sus: f32, rel: f32) {
        self.atk = atk; self.dec = dec; self.sus = sus.clamp(0.0, 1.0); self.rel = rel;
        self.exponential = false;
        self.inverse = [atk, dec, rel].map(|duration| if duration > 0.0 { 1.0 / duration } else { 0.0 });
    }

    /// Opt-in finite exponential decay/release; zero endpoint avoids a floor step.
    pub fn set_exponential(&mut self, atk: f32, dec: f32, sus: f32, rel: f32) {
        self.set(atk, dec, sus, rel);
        self.exponential = true;
    }
    #[inline]
    fn decay_curve(progress: f32) -> f32 {
        if progress >= 1.0 {
            return 0.0;
        }
        let scaled = progress * 256.0;
        let index = scaled as usize;
        let fraction = scaled - index as f32;
        let table = &super::preview_tables::DECAY;
        table[index] + (table[index + 1] - table[index]) * fraction
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

    #[cfg_attr(all(target_arch = "arm", target_os = "none"), unsafe(link_section = ".data.ram_func"))]
    pub fn tick(&mut self, dt: f32) -> f32 {
        match self.stage {
            Stage::Idle => self.level = 0.0,
            Stage::Attack => {
                self.level = if self.atk <= 0.0 {
                    1.0
                } else {
                    self.progress(self.atk, self.inverse[0])
                };
                self.t += dt;
                if self.t >= self.atk {
                    self.stage = Stage::Decay;
                    self.t = 0.0;
                }
            }
            Stage::Decay => {
                self.level = if self.dec <= 0.0 {
                    self.sus
                } else if self.exponential {
                    self.sus
                        + (1.0 - self.sus)
                            * Self::decay_curve(self.progress(self.dec, self.inverse[1]))
                } else {
                    1.0 - (1.0 - self.sus) * self.progress(self.dec, self.inverse[1])
                };
                self.t += dt;
                if self.t >= self.dec {
                    self.stage = Stage::Sustain;
                }
            }
            Stage::Sustain => self.level = self.sus,
            Stage::Release => {
                self.level = if self.rel <= 0.0 {
                    0.0
                } else if self.exponential {
                    self.rel_from * Self::decay_curve(self.progress(self.rel, self.inverse[2]))
                } else {
                    self.rel_from * (1.0 - self.progress(self.rel, self.inverse[2]))
                };
                self.t += dt;
                if self.t >= self.rel {
                    self.stage = Stage::Idle;
                    self.level = 0.0;
                }
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

#[cfg(test)]
mod exponential_tests {
    use super::*;
    fn curve(p: f32) -> f32 {
        let p = (p as f64).clamp(0., 1.);
        ((350f64.powf(-p) - 1. / 350.) / (1. - 1. / 350.)) as f32
    }
    #[test]
    fn baked_curve_matches_preview_floor_and_reaches_zero_continuously() {
        for i in 0..=10000 {
            let p = i as f32 / 10000.;
            let actual = Adsr::decay_curve(p);
            let preview = 350f64.powf(-(p as f64)) as f32;
            assert!((actual - curve(p)).abs() < 7e-5);
            assert!((actual - preview).abs() <= 1. / 350. + 7e-5);
        }
        assert_eq!(Adsr::decay_curve(0.), 1.);
        assert_eq!(Adsr::decay_curve(1.), 0.);
    }
    #[test]
    fn explicit_exponential_envelopes_cover_rates_gates_retrigger_and_old_linear_mode() {
        for sr in [8000., 22050., 44100., 48000., 96000.] {
            for tone in 0..=10 {
                for gate in [0.005, 0.05, 0.7] {
                    let end = 0.15 + 0.04 * tone as f32;
                    let mut a = Adsr::default();
                    a.set_exponential(0.01, end - 0.01, 0., end);
                    a.gate_on();
                    let dt = 1. / sr;
                    for n in 0..(sr * 0.9) as usize {
                        if n == (sr * gate) as usize {
                            a.gate_off();
                        }
                        if n == (sr * 0.75) as usize {
                            a.gate_on();
                        }
                        let expected = match a.stage {
                            Stage::Idle | Stage::Sustain => 0.,
                            Stage::Attack => (a.t / 0.01).min(1.),
                            Stage::Decay => curve(a.t / (end - 0.01)),
                            Stage::Release => a.rel_from * curve(a.t / end),
                        };
                        let old = a.level;
                        let stage = a.stage;
                        let sample = a.tick(dt);
                        assert!(sample.is_finite() && (0.0..=1.0).contains(&sample));
                        assert!(
                            (sample - if a.active() { expected } else { 0. }).abs() < 8e-5,
                            "sr={sr} tone={tone} n={n}"
                        );
                        if stage == Stage::Release && !a.active() {
                            assert!(old < 1e-3);
                        }
                    }
                    a.set(0.01, end, 0.4, end);
                    assert!(!a.exponential);
                }
            }
        }
    }
}

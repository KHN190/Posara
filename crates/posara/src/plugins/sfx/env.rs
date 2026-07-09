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

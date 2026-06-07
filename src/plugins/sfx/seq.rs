#[derive(Clone, Copy)]
pub struct Event {
    pub tick: u32,
    pub ch: usize,
    pub note: u8,
    pub vel: f32,
    pub dur: u32,
}

pub struct Sequencer {
    events: Vec<Event>,
    cursor: usize,
    pub spt: f32,
    elapsed: f32,
    pub playing: bool,
}

impl Sequencer {
    pub fn empty() -> Self {
        Self { events: Vec::new(), cursor: 0, spt: 1.0, elapsed: 0.0, playing: false }
    }

    pub fn load(&mut self, mut events: Vec<Event>, samples_per_tick: f32) {
        events.sort_by_key(|e| e.tick);
        self.events = events;
        self.cursor = 0;
        self.spt = samples_per_tick.max(1.0);
        self.elapsed = 0.0;
        self.playing = !self.events.is_empty();
    }

    pub fn stop(&mut self) {
        self.playing = false;
    }

    // advance one sample; push events whose start time has arrived into `out`.
    pub fn advance(&mut self, out: &mut Vec<Event>) {
        self.elapsed += 1.0;
        while self.cursor < self.events.len() {
            let ev = self.events[self.cursor];
            if (ev.tick as f32) * self.spt <= self.elapsed {
                out.push(ev);
                self.cursor += 1;
            } else {
                break;
            }
        }
        if self.cursor >= self.events.len() {
            self.playing = false;
        }
    }
}

pub fn note_to_freq(note: u8) -> f32 {
    440.0 * (2.0f32).powf((note as f32 - 69.0) / 12.0)
}

// One event packed per i64: tick:16 | ch:3 | note:8 | vol:7 | dur:16 (LSB→MSB).
pub fn unpack(word: i64) -> Event {
    let w = word as u64;
    Event {
        tick: (w & 0xFFFF) as u32,
        ch: ((w >> 16) & 0x7) as usize,
        note: ((w >> 19) & 0xFF) as u8,
        vel: ((w >> 27) & 0x7F) as f32 / 100.0,
        dur: ((w >> 34) & 0xFFFF) as u32,
    }
}

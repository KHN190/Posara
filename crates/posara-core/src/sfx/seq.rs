use alloc::vec::Vec;
use crate::math;

#[derive(Clone, Copy)]
pub struct Event {
    pub tick: u32,
    pub ch: usize,
    pub note: u8,
    pub vel: f32,
    pub dur: u32,
}

pub fn by_tick(mut events: Vec<Event>) -> Vec<Event> {
    events.sort_by_key(|e| e.tick);
    events
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

    pub fn load(&mut self, events: Vec<Event>, samples_per_tick: f32) -> Vec<Event> {
        let old = core::mem::replace(&mut self.events, events);
        self.cursor = 0;
        self.spt = samples_per_tick.max(1.0);
        self.elapsed = 0.0;
        self.playing = !self.events.is_empty();
        old
    }

    pub fn stop(&mut self) {
        self.playing = false;
    }

    pub fn tick(&mut self) {
        self.elapsed += 1.0;
    }

    pub fn next_due(&mut self) -> Option<Event> {
        match self.events.get(self.cursor).copied() {
            Some(ev) if (ev.tick as f32) * self.spt <= self.elapsed => {
                self.cursor += 1;
                Some(ev)
            }
            Some(_) => None,
            None => {
                self.playing = false;
                None
            }
        }
    }
}

pub fn note_to_freq(note: u8) -> f32 {
    440.0 * math::powf(2.0f32, (note as f32 - 69.0) / 12.0)
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

pub const EVENT_BYTES: usize = 8;

pub fn pack(tick: i64, channel: i64, note: i64, volume: i64, duration: i64) -> i64 {
    (tick & 0xffff) | ((channel & 7) << 16) | ((note & 0xff) << 19)
        | ((volume & 0x7f) << 27) | ((duration & 0xffff) << 34)
}

pub fn decode(bytes: &[u8]) -> Vec<Event> {
    let events = bytes.chunks(EVENT_BYTES).filter_map(|chunk| {
        let mut word = [0; EVENT_BYTES];
        word[..chunk.len()].copy_from_slice(chunk);
        let word = i64::from_le_bytes(word);
        (word != 0).then(|| unpack(word))
    }).collect();
    by_tick(events)
}

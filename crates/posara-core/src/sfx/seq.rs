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

pub fn by_tick(events: Vec<Event>) -> Vec<Event> {
    let mut indexed: Vec<_> = events.into_iter().enumerate().collect();
    indexed.sort_unstable_by_key(|(index, event)| (event.tick, *index));
    indexed.into_iter().map(|(_, event)| event).collect()
}

pub struct Sequencer {
    events: Vec<Event>,
    cursor: usize,
    /// Tempo edits take effect at the next tick boundary; load resets the phase.
    pub spt: f32,
    tick_index: u32,
    within: u32,
    interval: u32,
    fraction: f32,
    cached_spt: f32,
    pub playing: bool,
}

impl Sequencer {
    pub fn empty() -> Self {
        Self { events: Vec::new(), cursor: 0, spt: 1.0, tick_index: 0, within: 0, interval: 1, fraction: 0.0, cached_spt: 1.0, playing: false }
    }

    pub fn load(&mut self, events: Vec<Event>, samples_per_tick: f32) -> Vec<Event> {
        let old = core::mem::replace(&mut self.events, events);
        self.cursor = 0;
        self.spt = samples_per_tick.max(1.0).min(u32::MAX as f32);
        self.tick_index = 0;
        self.within = 0;
        self.fraction = 0.0;
        self.cached_spt = self.spt;
        self.advance_interval();
        self.playing = !self.events.is_empty();
        old
    }

    pub fn stop(&mut self) {
        self.playing = false;
    }

    pub fn tick(&mut self) {
        self.within += 1;
        if self.within >= self.interval {
            self.within = 0;
            self.tick_index = self.tick_index.saturating_add(1);
            self.advance_interval();
        }
    }

    fn advance_interval(&mut self) {
        if self.cached_spt != self.spt {
            self.spt = self.spt.max(1.0).min(u32::MAX as f32);
            self.cached_spt = self.spt;
        }
        let whole = self.spt as u32;
        let sum = self.fraction + (self.spt - whole as f32);
        let carry = sum as u32;
        let extra = carry + u32::from(sum - carry as f32 > 0.0) - u32::from(self.fraction > 0.0);
        self.interval = whole.saturating_add(extra).max(1);
        self.fraction = sum - carry as f32;
    }

    pub fn next_due(&mut self) -> Option<Event> {
        match self.events.get(self.cursor).copied() {
            Some(ev) if ev.tick <= self.tick_index => {
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

pub const EVENT_BYTES: usize = 8;

pub fn unpack_bytes(bytes: [u8; EVENT_BYTES]) -> Event {
    let lo = u32::from_le_bytes(bytes[..4].try_into().unwrap());
    let hi = u32::from_le_bytes(bytes[4..].try_into().unwrap());
    Event { tick: lo & 0xffff, ch: ((lo >> 16) & 7) as usize,
        note: ((lo >> 19) & 0xff) as u8,
        vel: (((lo >> 27) | (hi << 5)) & 0x7f) as f32 / 100.0,
        dur: (hi >> 2) & 0xffff }
}

pub fn pack_bytes(tick: u32, channel: u32, note: u32, volume: u32, duration: u32) -> [u8; EVENT_BYTES] {
    let lo = (tick & 0xffff) | ((channel & 7) << 16) | ((note & 0xff) << 19) | ((volume & 31) << 27);
    let hi = ((volume >> 5) & 3) | ((duration & 0xffff) << 2);
    let mut bytes = [0; EVENT_BYTES];
    bytes[..4].copy_from_slice(&lo.to_le_bytes());
    bytes[4..].copy_from_slice(&hi.to_le_bytes());
    bytes
}

#[cfg(not(all(target_os = "none", target_pointer_width = "32")))]
pub fn unpack(word: i64) -> Event { unpack_bytes(word.to_le_bytes()) }
#[cfg(not(all(target_os = "none", target_pointer_width = "32")))]
pub fn pack(tick: i64, channel: i64, note: i64, volume: i64, duration: i64) -> i64 {
    i64::from_le_bytes(pack_bytes(tick as u32, channel as u32, note as u32, volume as u32, duration as u32))
}

pub fn decode(bytes: &[u8]) -> Vec<Event> {
    by_tick(bytes.chunks(EVENT_BYTES).filter_map(|chunk| {
        let mut word = [0; EVENT_BYTES];
        word[..chunk.len()].copy_from_slice(chunk);
        word.iter().any(|&byte| byte != 0).then(|| unpack_bytes(word))
    }).collect())
}

#[cfg(test)]
mod clock_tests {
    use super::*;
    #[test]
    fn maximum_interval_and_tick_do_not_wrap() {
        let mut s = Sequencer::empty();
        s.interval = u32::MAX; s.within = u32::MAX - 1; s.tick_index = u32::MAX;
        s.tick(); assert_eq!(s.tick_index, u32::MAX); assert_eq!(s.within, 0);
    }
}

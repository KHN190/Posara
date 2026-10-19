#![no_std]

pub const CLOCK: u8 = 0xF8;
pub const START: u8 = 0xFA;
pub const STOP: u8 = 0xFC;

const CLOCKS_PER_PULSE: u8 = 12;
const HALF: u8 = CLOCKS_PER_PULSE / 2;

pub struct SyncIn {
    src: u8,
    last_us: Option<u64>,
    half_due_us: Option<u64>,
}

impl SyncIn {
    pub fn new(src: u8) -> Self { Self { src, last_us: None, half_due_us: None } }

    pub fn word(&self) -> u32 { CLOCK as u32 | (self.src as u32) << 24 }

    pub fn on_pulse(&mut self, now_us: u64) -> u8 {
        let flushed = if self.half_due_us.is_some() { HALF } else { 0 };
        self.half_due_us = Some(match self.last_us {
            Some(t) => now_us + now_us.saturating_sub(t) / 2,
            None => now_us,
        });
        self.last_us = Some(now_us);
        flushed + HALF
    }

    pub fn poll(&mut self, now_us: u64) -> u8 {
        match self.half_due_us {
            Some(due) if now_us >= due => { self.half_due_us = None; HALF }
            _ => 0,
        }
    }

    pub fn on_stop(&mut self) {
        self.last_us = None;
        self.half_due_us = None;
    }
}

pub struct SyncOut { n: u8 }

impl Default for SyncOut {
    fn default() -> Self { Self::new() }
}

impl SyncOut {
    pub fn new() -> Self { Self { n: 0 } }

    pub fn on_send(&mut self, status: u8) -> bool {
        match status {
            START => { self.n = 0; false }
            CLOCK => {
                let pulse = self.n == 0;
                self.n = (self.n + 1) % CLOCKS_PER_PULSE;
                pulse
            }
            _ => false,
        }
    }
}

const LISTEN_US: u64 = 1_000_000;
const SILENT_US: u64 = 3_000_000;

#[derive(Clone, Copy, PartialEq)]
enum Role { Listen(u64), Follow(u64), Lead }

pub struct Jack { plugged: bool, role: Role }

impl Default for Jack {
    fn default() -> Self { Self::new() }
}

impl Jack {
    pub fn new() -> Self { Self { plugged: false, role: Role::Lead } }

    pub fn leads(&self) -> bool { self.role == Role::Lead }

    pub fn plug(&mut self, plugged: bool, now_us: u64) {
        if plugged != self.plugged {
            self.plugged = plugged;
            self.role = if plugged { Role::Listen(now_us) } else { Role::Lead };
        }
    }

    pub fn on_pulse(&mut self, now_us: u64) { self.role = Role::Follow(now_us); }

    pub fn poll(&mut self, now_us: u64) {
        self.role = match self.role {
            Role::Listen(t) if now_us >= t + LISTEN_US => Role::Lead,
            Role::Follow(t) if now_us >= t + SILENT_US => Role::Lead,
            r => r,
        };
    }
}

use alloc::string::String;
use super::runtime::{Int, Word};
pub use crate::ports::{MidiPort, Imu, NullMidi, NullImu};

pub trait Clock {
    fn now_ms(&self) -> Word;
    // Rebase now() to 0 when a cart (re)loads.
    fn reset(&self) {}
}

pub struct NullClock;

impl Clock for NullClock {
    fn now_ms(&self) -> Word { 0 }
}

pub trait Presenter {
    fn configure(&mut self, w: usize, h: usize) -> Result<(), String>;
    // push RGB565 frame; returns surface-alive.
    fn present(&mut self, buf: &[u16]) -> Result<bool, String>;
    fn poll(&mut self) -> Option<(u8, u8)> { None }
    fn set_pos(&mut self, _x: Int, _y: Int) {}
}

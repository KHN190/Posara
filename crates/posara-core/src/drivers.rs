use alloc::string::String;

pub trait MidiPort {
    fn recv(&mut self) -> Option<u32>;
    fn send(&mut self, packed: u32) -> Result<(), String>;
}

pub trait Imu {
    fn accel(&mut self) -> [i32; 3];
}

pub struct NullMidi;

impl MidiPort for NullMidi {
    fn recv(&mut self) -> Option<u32> { None }
    fn send(&mut self, _packed: u32) -> Result<(), String> { Ok(()) }
}

pub struct NullImu;

impl Imu for NullImu {
    fn accel(&mut self) -> [i32; 3] { [0; 3] }
}

pub trait Clock {
    fn now_ms(&self) -> u64;
    // Rebase now() to 0 when a cart (re)loads.
    fn reset(&self) {}
}

pub struct NullClock;

impl Clock for NullClock {
    fn now_ms(&self) -> u64 { 0 }
}

pub trait Presenter {
    fn configure(&mut self, w: usize, h: usize) -> Result<(), String>;
    // push RGB565 frame; returns surface-alive.
    fn present(&mut self, buf: &[u16]) -> Result<bool, String>;
    fn poll(&mut self) -> Option<(u8, u8)> { None }
    fn set_pos(&mut self, _x: i64, _y: i64) {}
}

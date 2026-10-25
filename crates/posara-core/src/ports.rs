use alloc::string::String;

pub trait MidiPort {
    fn recv(&mut self) -> Option<u32>;
    fn send(&mut self, packed: u32) -> Result<(), String>;
}

pub trait Imu { fn accel(&mut self) -> [i32; 3]; }

pub struct NullMidi;
impl MidiPort for NullMidi {
    fn recv(&mut self) -> Option<u32> { None }
    fn send(&mut self, _: u32) -> Result<(), String> { Ok(()) }
}

pub struct NullImu;
impl Imu for NullImu { fn accel(&mut self) -> [i32; 3] { [0; 3] } }

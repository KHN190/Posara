pub use myriad::word32::{Float, Int, Value, VirtualMachine, Word};
pub(crate) use myriad::word32 as runtime;
#[path = "drivers.rs"]
pub mod drivers;

#[path = "abi.rs"]
pub mod abi;
#[path = "host.rs"]
pub mod host;
#[path = "plugin.rs"]
pub mod plugin;
#[path = "stepper.rs"]
pub mod stepper;
pub use host::{Drivers, Host};
#[cfg(feature = "sfx")]
#[path = "sfx/natives.rs"]
pub mod sfx_natives;
#[cfg(feature = "midi")]
#[path = "midi.rs"]
pub mod midi;
#[cfg(feature = "imu")]
#[path = "imu.rs"]
pub mod imu;
#[cfg(feature = "gfx")]
#[path = "gfx/mod.rs"]
pub mod gfx;
#[cfg(feature = "fs")]
#[path = "fs/mod.rs"]
pub mod fs;
#[cfg(feature = "net")]
#[path = "net.rs"]
pub mod net;

pub fn random_float(bits: u32) -> Float { (bits >> 8) as Float / 16_777_216.0 }
#[cfg(feature = "gfx")]
pub(crate) fn screen_configuration_format(_: Word, selected: u8) -> u8 { selected }

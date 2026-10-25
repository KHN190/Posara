pub use myriad::word64::{Float, Int, Value, VirtualMachine, Word};
pub(crate) use myriad::word64 as runtime;
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

pub fn random_float(bits: u32) -> Float { bits as Float / 4_294_967_296.0 }
#[cfg(feature = "gfx")]
pub(crate) fn screen_configuration_format(packed: Word, _: u8) -> u8 { ((packed >> 32) & 0xff) as u8 }

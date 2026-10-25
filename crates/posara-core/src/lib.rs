#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;
mod runtime_stepper;
pub use runtime_stepper::RuntimeStepper;

pub mod word32;
#[cfg(not(all(target_os = "none", target_pointer_width = "32")))]
pub mod word64;
#[cfg(not(all(target_os = "none", target_pointer_width = "32")))]
pub use word64::{abi, host, plugin, stepper};
#[cfg(all(target_os = "none", target_pointer_width = "32"))]
pub use word32::{abi, host, plugin, stepper};
pub mod audio;
pub mod devices;
mod ports;
#[cfg(not(all(target_os = "none", target_pointer_width = "32")))]
pub use word64::drivers;
#[cfg(all(target_os = "none", target_pointer_width = "32"))]
pub use word32::drivers;
#[cfg(all(feature = "fs", not(all(target_os = "none", target_pointer_width = "32"))))]
pub use word64::fs;
#[cfg(all(feature = "fs", target_os = "none", target_pointer_width = "32"))]
pub use word32::fs;
#[cfg(all(feature = "gfx", not(all(target_os = "none", target_pointer_width = "32"))))]
pub use word64::gfx;
#[cfg(all(feature = "gfx", target_os = "none", target_pointer_width = "32"))]
pub use word32::gfx;
#[cfg(all(feature = "imu", not(all(target_os = "none", target_pointer_width = "32"))))]
pub use word64::imu;
#[cfg(all(feature = "imu", target_os = "none", target_pointer_width = "32"))]
pub use word32::imu;
#[cfg(any(feature = "sfx", feature = "gfx"))]
pub mod math;
#[cfg(all(feature = "midi", not(all(target_os = "none", target_pointer_width = "32"))))]
pub use word64::midi;
#[cfg(all(feature = "midi", target_os = "none", target_pointer_width = "32"))]
pub use word32::midi;
#[cfg(all(feature = "net", not(all(target_os = "none", target_pointer_width = "32"))))]
pub use word64::net;
#[cfg(all(feature = "net", target_os = "none", target_pointer_width = "32"))]
pub use word32::net;
#[cfg(feature = "sfx")]
pub mod sfx;

#[allow(unused_imports)]
pub(crate) mod prelude {
    pub use alloc::boxed::Box;
    pub use alloc::format;
    pub use alloc::rc::Rc;
    pub use alloc::string::{String, ToString};
    pub use alloc::vec;
    pub use alloc::vec::Vec;
}

#[macro_export]
#[doc(hidden)]
macro_rules! warn {
    ($($t:tt)*) => {{
        #[cfg(feature = "std")]
        std::eprintln!($($t)*);
        #[cfg(not(feature = "std"))]
        let _ = core::format_args!($($t)*);
    }};
}

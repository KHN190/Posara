#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

pub mod abi;
pub mod audio;
pub mod devices;
pub mod drivers;
pub mod host;
pub mod plugin;
pub mod stepper;
#[cfg(feature = "fs")]
pub mod fs;
#[cfg(feature = "gfx")]
pub mod gfx;
#[cfg(feature = "imu")]
pub mod imu;
#[cfg(any(feature = "sfx", feature = "gfx"))]
pub mod math;
#[cfg(feature = "midi")]
pub mod midi;
#[cfg(feature = "net")]
pub mod net;
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

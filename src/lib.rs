pub mod plugin;
pub mod plugins;
pub mod debug;
pub mod host;
pub mod lint;
#[cfg(feature = "gfx")]
pub mod profile;
pub mod runner;

// Domain modules live under plugins/; these aliases keep the old paths.
#[cfg(feature = "gfx")]
pub use plugins::gfx;
#[cfg(feature = "gfx")]
pub use plugins::gfx::input;
#[cfg(feature = "sfx")]
pub use plugins::sfx;
#[cfg(feature = "fs")]
pub use plugins::fs;

#[cfg(feature = "gfx")]
pub use gfx::Framebuffer;
#[cfg(feature = "gfx")]
pub use input::{Button, Controller};
#[cfg(feature = "sfx")]
pub use sfx::{Audio, Mixer};
pub use host::Host;

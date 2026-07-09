#[cfg(all(feature = "gfx-desktop", feature = "gfx-web"))]
compile_error!("pick one gfx backend: gfx-desktop or gfx-web");
#[cfg(all(feature = "sfx-desktop", feature = "sfx-web"))]
compile_error!("pick one sfx backend: sfx-desktop or sfx-web");

pub mod backend;
pub mod devices;
pub mod plugin;
pub mod plugins;
pub mod debug;
pub mod host;
pub mod lint;
#[cfg(feature = "gfx")]
#[cfg(feature = "gfx-desktop")]
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
pub use myriad::VirtualMachine;
pub use polka::Module;

// single source for the version string (Cargo.toml).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(feature = "gfx")]
pub mod gfx;
#[cfg(feature = "sfx")]
pub mod sfx;
#[cfg(feature = "midi-desktop")]
pub mod midi;
#[cfg(feature = "net")]
pub mod net;

#[cfg(feature = "sfx")]
pub use sfx::SfxPlugin;
#[cfg(feature = "midi-desktop")]
pub use midi::MidiPlugin;

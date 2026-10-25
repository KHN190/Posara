pub use posara_core::sfx::{env, mixer, natives, seq, spsc};
#[cfg(feature = "synth")]
pub use posara_core::sfx::synth;
pub mod output;
pub mod record;

pub use mixer::Mixer;

pub const DELAY_CAP_MS: u32 = 2000;
pub use natives::{push, CmdProd};
pub use output::{Audio, AudioMeter, AudioSnapshot, Meter, SampleRing};
pub use record::{Recorder, write_wav_f32};

pub(crate) use crate::{runtime, profile_core};
mod plugin;
pub use plugin::{SfxPlugin, RecorderSlot};
#[cfg(feature="fs")]
pub use plugin::register_record_natives;

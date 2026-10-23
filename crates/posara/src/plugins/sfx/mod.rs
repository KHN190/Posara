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

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
#[cfg(feature = "fs")]
use std::sync::Arc;
#[cfg(feature = "fs")]
use std::sync::atomic::AtomicBool;

use myriad::VirtualMachine;
#[cfg(feature = "fs")]
use myriad::{NativeCtx, Value};
#[cfg(feature = "fs")]
use myriad::read_string;

use posara_core::plugin::Plugin;

pub type RecorderSlot = Rc<RefCell<Option<Recorder>>>;

pub struct SfxPlugin {
    pub audio: Audio,
    pub recorder: RecorderSlot,
    #[cfg_attr(not(feature = "fs"), allow(dead_code))]
    root: PathBuf,
}

impl SfxPlugin {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        Self::with_audio(root, false)
    }

    // muted = no audio device (silent run, like --headless for sound).
    pub fn with_audio(root: PathBuf, muted: bool) -> Result<Self, String> {
        #[cfg(feature = "sfx-desktop")]
        let audio = if muted { Audio::silent()? } else { Audio::new()? };
        #[cfg(not(feature = "sfx-desktop"))]
        let audio = if muted { Audio::silent()? } else { Audio::web(44100)? };
        Ok(Self {
            audio,
            recorder: Rc::new(RefCell::new(None)),
            root,
        })
    }

    pub fn drain_recorder(&self) {
        if let Some(r) = self.recorder.borrow_mut().as_mut() {
            if let Err(e) = r.drain() {
                eprintln!("recorder drain: {e}");
            }
        }
    }
}

impl Drop for SfxPlugin {
    fn drop(&mut self) {
        if let Some(r) = self.recorder.borrow_mut().as_mut() {
            let _ = r.stop();
        }
    }
}

impl Plugin for SfxPlugin {
    fn install(&self, vm: &mut VirtualMachine) {
        #[cfg(not(feature = "fs"))]
        let _ = vm;
        #[cfg(feature = "fs")]
        register_record_natives(
            vm,
            Arc::clone(&self.audio.rec_ring),
            Arc::clone(&self.audio.rec_on),
            self.audio.sample_rate,
            self.audio.channels,
            self.root.clone(),
            Rc::clone(&self.recorder),
        );
    }
}

#[cfg(feature = "fs")]
pub fn register_record_natives(
    vm: &mut VirtualMachine,
    ring: SampleRing,
    enabled: Arc<AtomicBool>,
    sample_rate: u32,
    channels: u16,
    root: PathBuf,
    rec: RecorderSlot,
) -> Vec<&'static str> {
    let r = root.clone();
    let rs = Rc::clone(&rec);
    let rg = Arc::clone(&ring);
    let en = Arc::clone(&enabled);
    let start = Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let Some(rel) = read_string(ctx.heap, posara_core::abi::Args(a).handle(0)) else {
            return Ok((Value::from_int(-1), false));
        };
        let Some(path) = crate::fs::resolve(&r, &rel) else {
            return Ok((Value::from_int(-1), false));
        };
        if rs.borrow().is_some() {
            return Ok((Value::from_int(-1), false));
        }
        match Recorder::start(&path, sample_rate, channels, Arc::clone(&rg), Arc::clone(&en)) {
            Ok(r) => { *rs.borrow_mut() = Some(r); Ok((Value::from_int(0), false)) }
            Err(e) => { eprintln!("mx_record_start: {e}"); Ok((Value::from_int(-1), false)) }
        }
    });
    vm.register_native("snd_bus_record_start", start);
    let rs = Rc::clone(&rec);
    let stop = Rc::new(move |_ctx: &mut NativeCtx, _a: &[Value]| {
        let mut slot = rs.borrow_mut();
        let Some(r) = slot.as_mut() else {
            return Ok((Value::from_int(-1), false));
        };
        let result = match r.stop() {
            Ok(()) => 0,
            Err(e) => { eprintln!("mx_record_stop: {e}"); -1 }
        };
        *slot = None;
        Ok((Value::from_int(result), false))
    });
    vm.register_native("snd_bus_record_stop", stop);
    vec!["snd_bus_record_start", "snd_bus_record_stop"]
}

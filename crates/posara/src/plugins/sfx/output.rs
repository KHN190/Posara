use std::sync::Arc;
use std::sync::atomic::AtomicBool;
#[cfg(feature = "sfx-desktop")]
use std::sync::atomic::Ordering;

pub use posara_core::sfx::meter::{AudioMeter, AudioSnapshot, Meter};

#[cfg(feature = "sfx-desktop")]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

#[cfg(feature = "sfx-desktop")]
use crate::sfx::mixer::Cmd;
use crate::sfx::mixer::{Mixer, Retired};
use crate::sfx::spsc::Spsc;
use crate::sfx::CmdProd;

pub type SampleRing = Arc<Spsc<f32>>;
pub struct Audio {
    pub cmds: CmdProd,
    pub rec_ring: SampleRing,
    pub rec_on: Arc<AtomicBool>,
    pub meter: Meter,
    pub sample_rate: u32,
    pub channels: u16,
    retired: Arc<Spsc<Retired>>,
    #[cfg(feature = "sfx-desktop")]
    _stream: Option<cpal::Stream>,
    #[cfg(not(feature = "sfx-desktop"))]
    mixer: std::rc::Rc<std::cell::RefCell<Mixer>>,
}

impl Audio {
    // No device: commands queue and drop, everything else runs. No recording.
    pub fn silent() -> Result<Self, String> {
        let meter: Meter = Arc::new(AudioMeter::default());
        #[cfg(not(feature = "sfx-desktop"))]
        let mixer = Mixer::new(44100, 2, Arc::clone(&meter), super::DELAY_CAP_MS);
        Ok(Self {
            #[cfg(not(feature = "sfx-desktop"))]
            retired: mixer.retired(),
            #[cfg(feature = "sfx-desktop")]
            retired: Arc::new(Spsc::new(1)),
            cmds: Arc::new(Spsc::new(1024)),
            rec_ring: Arc::new(Spsc::new(8192)),
            rec_on: Arc::new(AtomicBool::new(false)),
            #[cfg(not(feature = "sfx-desktop"))]
            mixer: std::rc::Rc::new(std::cell::RefCell::new(mixer)),
            meter,
            sample_rate: 44100,
            channels: 2,
            #[cfg(feature = "sfx-desktop")]
            _stream: None,
        })
    }

    // Web sink: mixer lives here; the AudioWorklet drives it via pull().
    #[cfg(not(feature = "sfx-desktop"))]
    pub fn web(sample_rate: u32) -> Result<Self, String> {
        let meter: Meter = Arc::new(AudioMeter::default());
        let mixer = Mixer::new(sample_rate, 2, Arc::clone(&meter), super::DELAY_CAP_MS);
        Ok(Self {
            cmds: Arc::new(Spsc::new(1024)),
            rec_ring: Arc::new(Spsc::new(8192)),
            rec_on: Arc::new(AtomicBool::new(false)),
            retired: mixer.retired(),
            mixer: std::rc::Rc::new(std::cell::RefCell::new(mixer)),
            meter,
            sample_rate,
            channels: 2,
        })
    }

    pub fn reclaim(&self) {
        while self.retired.try_pop().is_some() {}
    }

    pub fn share(&self) -> Self {
        Self {
            cmds: Arc::clone(&self.cmds),
            rec_ring: Arc::clone(&self.rec_ring),
            rec_on: Arc::clone(&self.rec_on),
            meter: Arc::clone(&self.meter),
            sample_rate: self.sample_rate,
            channels: self.channels,
            retired: Arc::clone(&self.retired),
            #[cfg(feature = "sfx-desktop")]
            _stream: None,
            #[cfg(not(feature = "sfx-desktop"))]
            mixer: std::rc::Rc::clone(&self.mixer),
        }
    }

    #[cfg(not(feature = "sfx-desktop"))]
    pub fn pull(&mut self, out: &mut [f32]) {
        let mut mixer = self.mixer.borrow_mut();
        while let Some(cmd) = self.cmds.try_pop() { mixer.apply(cmd); }
        mixer.mix(out);
    }

    #[cfg(feature = "sfx-desktop")]
    pub fn new() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or("no output device")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let sample_format = supported.sample_format();
        let sample_rate = supported.sample_rate().0;
        let out_channels = supported.channels() as usize;
        let stream_config: cpal::StreamConfig = supported.into();

        let cmds: Arc<Spsc<Cmd>> = Arc::new(Spsc::new(1024));
        let cmds_cb = Arc::clone(&cmds);
        let rec_ring: SampleRing = Arc::new(Spsc::new(8192));
        let rec_ring_cb = Arc::clone(&rec_ring);
        let rec_on = Arc::new(AtomicBool::new(false));
        let rec_on_cb = Arc::clone(&rec_on);
        let meter: Meter = Arc::new(AudioMeter::default());
        let mut mixer = Mixer::new(sample_rate, out_channels, Arc::clone(&meter), super::DELAY_CAP_MS);
        let retired = mixer.retired();
        let err_fn = |e| eprintln!("audio stream error: {e}");

        let stream = match sample_format {
            cpal::SampleFormat::F32 => device.build_output_stream(
                &stream_config,
                move |out: &mut [f32], _| {
                    while let Some(cmd) = cmds_cb.try_pop() { mixer.apply(cmd); }
                    mixer.mix(out);
                    if rec_on_cb.load(Ordering::Relaxed) {
                        for &s in out.iter() { let _ = rec_ring_cb.try_push(s); }
                    }
                },
                err_fn, None,
            ).map_err(|e| e.to_string())?,
            other => return Err(format!("unsupported sample format: {other:?}")),
        };
        stream.play().map_err(|e| e.to_string())?;
        Ok(Self {
            cmds, rec_ring, rec_on, meter, retired,
            sample_rate, channels: out_channels as u16,
            _stream: Some(stream),
        })
    }
}

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::sfx::mixer::{Cmd, Mixer};
use crate::sfx::spsc::Spsc;

pub type CmdProd = Arc<Spsc<Cmd>>;
pub type SampleRing = Arc<Spsc<f32>>;
pub type Meter = Arc<AudioMeter>;

// Audio-thread → main-thread telemetry for the profiler's sound panel.
// Written each buffer by the mixer/callback, read at frame rate.
#[derive(Default)]
pub struct AudioMeter {
    out_peak: AtomicU32,             // master post-mix peak, f32 bits
    notes: AtomicU32,                // total synth note-ons applied (monotonic)
    ch_peak: [AtomicU32; 4],         // per-pid output peak, f32 bits
    ch_voices: [AtomicU32; 4],       // per-pid active voices
}

pub struct AudioSnapshot {
    pub notes: u32,
    pub out_peak: f32,
    pub ch_peak: [f32; 4],
    pub ch_voices: [u32; 4],
}

impl AudioMeter {
    pub fn set_out(&self, peak: f32) { self.out_peak.store(peak.to_bits(), Ordering::Relaxed); }
    pub fn inc_notes(&self) { self.notes.fetch_add(1, Ordering::Relaxed); }
    pub fn set_channels(&self, peak: [f32; 4], voices: [u32; 4]) {
        for i in 0..4 {
            self.ch_peak[i].store(peak[i].to_bits(), Ordering::Relaxed);
            self.ch_voices[i].store(voices[i], Ordering::Relaxed);
        }
    }
    pub fn snapshot(&self) -> AudioSnapshot {
        let mut ch_peak = [0.0; 4];
        let mut ch_voices = [0; 4];
        for i in 0..4 {
            ch_peak[i] = f32::from_bits(self.ch_peak[i].load(Ordering::Relaxed));
            ch_voices[i] = self.ch_voices[i].load(Ordering::Relaxed);
        }
        AudioSnapshot {
            notes: self.notes.load(Ordering::Relaxed),
            out_peak: f32::from_bits(self.out_peak.load(Ordering::Relaxed)),
            ch_peak,
            ch_voices,
        }
    }
}

pub struct Audio {
    pub cmds: CmdProd,
    pub rec_ring: SampleRing,
    pub rec_on: Arc<AtomicBool>,
    pub meter: Meter,
    pub sample_rate: u32,
    pub channels: u16,
    _stream: cpal::Stream,
}

impl Audio {
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
        let mut mixer = Mixer::new(sample_rate, out_channels, Arc::clone(&meter));
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
            cmds, rec_ring, rec_on, meter,
            sample_rate, channels: out_channels as u16,
            _stream: stream,
        })
    }
}

pub fn push(cmds: &CmdProd, cmd: Cmd) {
    let _ = cmds.try_push(cmd);
}

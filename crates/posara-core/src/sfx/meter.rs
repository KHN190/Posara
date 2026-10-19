use alloc::sync::Arc;
use core::sync::atomic::{AtomicU32, Ordering};

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

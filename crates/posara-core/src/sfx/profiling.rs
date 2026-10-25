use core::sync::atomic::{AtomicU32, Ordering::Relaxed};

#[derive(Clone, Copy)]
#[repr(usize)]
pub enum Stage { Setup, Commands, Sequence, LegacyVoices, SynthVoice, SynthPan, SynthFilter, SynthFx, Sample, MasterFx, Output, Wait }
pub const NAMES: [&str; 12] = ["setup", "commands", "sequence", "legacy_voices", "synth_voice", "synth_pan", "synth_filter", "synth_fx", "sample", "master_fx", "output", "dma_wait"];
static STAGE: AtomicU32 = AtomicU32::new(0);
static TOTAL: AtomicU32 = AtomicU32::new(0);
static OUTSIDE: AtomicU32 = AtomicU32::new(0);
static COUNTS: [AtomicU32; 12] = [const { AtomicU32::new(0) }; 12];
static VOICES: [AtomicU32; 5] = [const { AtomicU32::new(0) }; 5];

#[inline]
pub fn mark(stage: Stage) { STAGE.store(stage as u32, Relaxed); }

fn increment(counter: &AtomicU32) {
    let _ = counter.fetch_update(Relaxed, Relaxed, |n| Some(n.saturating_add(1)));
}

pub fn sample(in_range: bool) {
    increment(&TOTAL);
    if !in_range { increment(&OUTSIDE); }
    increment(&COUNTS[STAGE.load(Relaxed) as usize]);
}

pub fn voices(pool: u32, active: u32, fm: u32, lfo: u32, bitcrush: u32) {
    for (slot, value) in VOICES.iter().zip([pool, active, fm, lfo, bitcrush]) { slot.store(value, Relaxed); }
}

pub struct Snapshot { pub total: u32, pub out_of_range: u32, pub counts: [u32; 12], pub voices: [u32; 5] }
pub fn snapshot() -> Snapshot {
    Snapshot { total: TOTAL.load(Relaxed), out_of_range: OUTSIDE.load(Relaxed), counts: core::array::from_fn(|i| COUNTS[i].load(Relaxed)), voices: core::array::from_fn(|i| VOICES[i].load(Relaxed)) }
}

static LIMIT_CYCLES: AtomicU32 = AtomicU32::new(0);
pub fn limiter_cycles() -> u32 { LIMIT_CYCLES.load(Relaxed) }
pub fn set_limiter_cycles(value: u32) { LIMIT_CYCLES.store(value, Relaxed); }
#[inline]
pub fn cycles() -> u32 {
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    unsafe { core::ptr::read_volatile(0xe0001004 as *const u32) }
    #[cfg(not(all(target_arch = "arm", target_os = "none")))]
    { 0 }
}

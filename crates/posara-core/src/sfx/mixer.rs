use alloc::vec::Vec;
use crate::math;

use alloc::sync::Arc;
use crate::sfx::spsc::Spsc;
use crate::sfx::meter::Meter;
use crate::sfx::sample::SamplePlayer;
use crate::sfx::seq::{note_to_freq, Event, Sequencer};
use crate::sfx::voice::{Voice, WAVE_NOISE, WAVE_SQUARE};

#[inline]
#[cfg(any(test, all(target_os = "none", target_pointer_width = "32")))]
fn approximate_limit_sample(x: f32) -> f32 {
    if x.abs() > 1.0 || !x.is_finite() { return math::tanh(x); }
    let z = x * x;
    x * (135135.0 + z * (17325.0 + z * (378.0 + z)))
        / (135135.0 + z * (62370.0 + z * (3150.0 + z * 28.0)))
}

#[inline]
fn limit_sample(x: f32) -> f32 {
    #[cfg(all(target_os = "none", target_pointer_width = "32"))]
    { approximate_limit_sample(x) }
    #[cfg(not(all(target_os = "none", target_pointer_width = "32")))]
    { math::tanh(x) }
}

fn limit_stereo(l: f32, r: f32) -> (f32, f32) {
    let left = limit_sample(l * 0.8);
    let right = if l.to_bits() == r.to_bits() { left } else { limit_sample(r * 0.8) };
    (left, right)
}

// VM thread → audio thread commands, carried over a lock-free SPSC ring
pub enum Cmd {
    Inst(usize, u8, u32, u32, f32, u32),
    Pan(usize, f32, f32),
    Fx(usize, u8, f32, f32),
    Lfo(usize, u8, u8, f32, f32),
    Play(usize, f32, f32, u32),
    PlayMidi(usize, i32, f32, u32),
    Off(usize),
    Tone(f32, u32, f32, u32),
    Noise(u32, f32, u32),
    Wave(u8, f32, u32, f32, u32),
    Seq(Vec<Event>, u32),
    SeqStop,
    Sample(Vec<u8>, f32, f32),
    SampleStop,
    BusDelay(u32, f32, f32),
    BusReverb(f32, f32, f32),
    #[cfg(feature = "synth")]
    SynOsc(usize, usize, u8, i32, f32, f32),
    #[cfg(feature = "synth")]
    SynFilter(usize, u8, f32, f32),
    #[cfg(feature = "synth")]
    SynEnv(usize, usize, u8, f32, f32, f32, f32, f32),
    #[cfg(feature = "synth")]
    SynLfo(usize, u8, u8, f32, f32),
    #[cfg(feature = "synth")]
    SynPan(usize, f32),
    #[cfg(feature = "synth")]
    SynUnison(usize, u8, f32),
    #[cfg(feature = "synth")]
    SynFx(usize, u8, f32, f32),
    #[cfg(feature = "synth")]
    SynVoices(usize),
    #[cfg(feature = "synth")]
    SynOn(usize, u8, f32, u32),
    #[cfg(feature = "synth")]
    SynOff(usize, u8),
    #[cfg(feature = "synth")]
    SynStop(usize),
    #[cfg(feature = "synth")]
    SynPanic,
}

const RETIRE_CAP: usize = 16;

pub enum Retired {
    Seq(Vec<Event>),
    Sample(Vec<u8>),
}

pub struct Mixer {
    pub voices: [Voice; 4],
    pub seq: Sequencer,
    pub sample: SamplePlayer,
    pub sample_rate: u32,
    pub out_channels: usize,
    meter: Meter,
    retired: Arc<Spsc<Retired>>,
    delay: super::bus::Delay,
    reverb: Option<super::bus::Reverb>,
    #[cfg(feature = "synth")]
    synth: super::synth::Synth,
}

impl Mixer {
    pub fn new(sample_rate: u32, out_channels: usize, meter: Meter, delay_cap_ms: u32) -> Self {
        Self::new_with_reverb(sample_rate, out_channels, meter, delay_cap_ms, true)
    }

    pub fn new_with_reverb(sample_rate: u32, out_channels: usize, meter: Meter, delay_cap_ms: u32, reverb: bool) -> Self {
        let cap = duration_samples(delay_cap_ms, sample_rate) as usize;
        Self {
            voices: [Voice::new(); 4],
            seq: Sequencer::empty(),
            sample: SamplePlayer::default(),
            sample_rate,
            out_channels,
            meter,
            retired: Arc::new(Spsc::new(RETIRE_CAP)),
            delay: super::bus::Delay::new(cap, out_channels != 1),
            reverb: reverb.then(|| super::bus::Reverb::new(sample_rate as f32)),
            #[cfg(feature = "synth")]
            synth: super::synth::Synth::new(),
        }
    }

    pub fn play_sample(&mut self, bytes: Vec<u8>, rate: f32, vol: f32) {
        let old = self.sample.play(bytes, rate, vol);
        if old.capacity() > 0 { self.retire(Retired::Sample(old)); }
    }

    pub fn retired(&self) -> Arc<Spsc<Retired>> { Arc::clone(&self.retired) }

    // Full ring = the VM skipped reclaiming for a frame; dropping here is the documented fallback.
    fn retire(&self, r: Retired) {
        let _ = self.retired.try_push(r);
    }

    pub fn apply(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Inst(ch, w, a, d, s, r) => self.inst(ch, w, a, d, s, r),
            Cmd::Pan(ch, l, r) => self.pan(ch, l, r),
            Cmd::Fx(ch, k, amt, p) => self.fx(ch, k, amt, p),
            Cmd::Lfo(ch, t, w, rate, dep) => self.lfo(ch, t, w, rate, dep),
            Cmd::Play(ch, f, v, dur) => self.play(ch, f, v, dur),
            Cmd::PlayMidi(ch, n, v, dur) => self.play_midi(ch, n, v, dur),
            Cmd::Off(ch) => self.off(ch),
            Cmd::Tone(f, dur, v, dec) => self.tone(f, dur, v, dec),
            Cmd::Noise(dur, v, dec) => self.noise(dur, v, dec),
            Cmd::Wave(k, f, dur, v, dec) => self.wave(k, f, dur, v, dec),
            Cmd::Seq(ev, mpt) => self.play_seq(ev, mpt),
            Cmd::SeqStop => self.seq.stop(),
            Cmd::Sample(b, rate, v) => self.play_sample(b, rate, v),
            Cmd::SampleStop => self.sample.stop(),
            Cmd::BusDelay(ms, fb, mix) => self.delay.set(ms, fb, mix, self.sample_rate as f32),
            Cmd::BusReverb(room, damp, mix) => { if let Some(reverb) = &mut self.reverb { reverb.set(room, damp, mix); } },
            #[cfg(feature = "synth")]
            Cmd::SynOsc(pid, idx, w, semi, fine, lvl) => self.synth.osc(pid, idx, w, semi, fine, lvl),
            #[cfg(feature = "synth")]
            Cmd::SynFilter(pid, k, c, r) => self.synth.filter(pid, k, c, r),
            #[cfg(feature = "synth")]
            Cmd::SynEnv(pid, slot, t, d, a, dec, s, r) => self.synth.env(pid, slot, t, d, a, dec, s, r),
            #[cfg(feature = "synth")]
            Cmd::SynLfo(pid, t, w, rate, dep) => self.synth.lfo(pid, t, w, rate, dep),
            #[cfg(feature = "synth")]
            Cmd::SynPan(pid, pos) => self.synth.pan(pid, pos),
            #[cfg(feature = "synth")]
            Cmd::SynUnison(pid, c, det) => self.synth.unison(pid, c, det),
            #[cfg(feature = "synth")]
            Cmd::SynFx(pid, k, amt, p) => self.synth.fx(pid, k, amt, p),
            #[cfg(feature = "synth")]
            Cmd::SynVoices(n) => self.synth.set_voices(n),
            #[cfg(feature = "synth")]
            Cmd::SynOn(pid, note, vel, dur) => {
                let gate = if dur == 0 { 0 } else { self.ms_to_samples(dur) };
                self.synth.note_on(pid, note, vel, gate);
                self.meter.inc_notes();
            }
            #[cfg(feature = "synth")]
            Cmd::SynOff(pid, note) => self.synth.note_off(pid, note),
            #[cfg(feature = "synth")]
            Cmd::SynStop(pid) => self.synth.stop(pid),
            #[cfg(feature = "synth")]
            Cmd::SynPanic => self.synth.panic(),
        }
    }

    fn ms_to_samples(&self, ms: u32) -> u32 {
        duration_samples(ms, self.sample_rate)
    }

    pub fn inst(&mut self, ch: usize, wave: u8, atk: u32, dec: u32, sus: f32, rel: u32) {
        if let Some(v) = self.voices.get_mut(ch) {
            v.set_inst(wave, atk as f32 / 1000.0, dec as f32 / 1000.0, sus, rel as f32 / 1000.0);
        }
    }

    pub fn pan(&mut self, ch: usize, l: f32, r: f32) {
        if let Some(v) = self.voices.get_mut(ch) { v.set_pan(l, r); }
    }

    pub fn fx(&mut self, ch: usize, kind: u8, amt: f32, param: f32) {
        if let Some(v) = self.voices.get_mut(ch) { v.fx.set(kind, amt, param); }
    }

    pub fn lfo(&mut self, ch: usize, target: u8, wave: u8, rate: f32, depth: f32) {
        if let Some(v) = self.voices.get_mut(ch) { v.lfo.set(target, wave, rate, depth); }
    }

    pub fn play(&mut self, ch: usize, freq: f32, vel: f32, dur_ms: u32) {
        let gate = if dur_ms == 0 { 0 } else { self.ms_to_samples(dur_ms) };
        if let Some(v) = self.voices.get_mut(ch) { v.play(freq, vel, gate); }
    }

    pub fn play_midi(&mut self, ch: usize, note: i32, vel: f32, dur_ms: u32) {
        self.play(ch, note_to_freq(note.clamp(0, 127) as u8), vel, dur_ms);
    }

    pub fn off(&mut self, ch: usize) {
        if let Some(v) = self.voices.get_mut(ch) { v.off(); }
    }

    pub fn play_seq(&mut self, events: Vec<Event>, ms_per_tick: u32) {
        let spt = (ms_per_tick as f32) * (self.sample_rate as f32) / 1000.0;
        let old = self.seq.load(events, spt);
        if old.capacity() > 0 { self.retire(Retired::Seq(old)); }
    }

    // legacy fire-and-forget: pick a free voice, default patch (decay = release).
    fn legacy(&mut self, wave: u8, freq: f32, dur_ms: u32, vol: f32, decay_ms: u32) {
        let slot = self.voices.iter().position(|v| !v.active()).unwrap_or(0);
        let gate = if dur_ms == 0 { 0 } else { self.ms_to_samples(dur_ms) };
        let v = &mut self.voices[slot];
        v.set_pan(1.0, 1.0);
        v.fx.set(0, 0.0, 0.0);
        v.lfo.set(0, 0, 0.0, 0.0);
        v.set_inst(wave, 0.0, 0.0, 1.0, decay_ms as f32 / 1000.0);
        v.play(freq, vol, gate);
    }

    pub fn tone(&mut self, freq: f32, dur_ms: u32, vol: f32, decay_ms: u32) {
        self.legacy(WAVE_SQUARE, freq, dur_ms, vol, decay_ms);
    }
    pub fn noise(&mut self, dur_ms: u32, vol: f32, decay_ms: u32) {
        self.legacy(WAVE_NOISE, 0.0, dur_ms, vol, decay_ms);
    }
    pub fn wave(&mut self, kind: u8, freq: f32, dur_ms: u32, vol: f32, decay_ms: u32) {
        self.legacy(kind, freq, dur_ms, vol, decay_ms);
    }

    fn advance_seq(&mut self) {
        if !self.seq.playing { return; }
        self.seq.tick();
        while let Some(ev) = self.seq.next_due() {
            let ms_per_tick = self.seq.spt * 1000.0 / self.sample_rate as f32;
            self.play(ev.ch, note_to_freq(ev.note), ev.vel, (ev.dur as f32 * ms_per_tick) as u32);
        }
    }

    #[cfg_attr(all(target_arch = "arm", target_os = "none"), unsafe(link_section = ".data.ram_func"))]
    #[cfg_attr(all(target_arch = "arm", target_os = "none"), inline(never))]
    pub fn mix(&mut self, out: &mut [f32]) {
        let nc = self.out_channels.max(1);
        let sr = self.sample_rate as f32;
        let mut out_peak = 0.0f32;
        let mut ch_peak = [0.0f32; 4];
        #[cfg(feature = "audio-profile")]
        let mut limit_cycles = 0u32;
        for frame in out.chunks_mut(nc) {
            #[cfg(feature = "audio-profile")]
            super::profiling::mark(super::profiling::Stage::Sequence);
            self.advance_seq();
            let (mut l, mut r) = (0.0f32, 0.0f32);
            #[cfg(feature = "audio-profile")]
            super::profiling::mark(super::profiling::Stage::LegacyVoices);
            for (i, v) in self.voices.iter_mut().enumerate() {
                if !v.active() { continue; }
                let (vl, vr) = v.tick(sr);
                l += vl; r += vr;
                let a = vl.abs().max(vr.abs());
                if a > ch_peak[i] { ch_peak[i] = a; }
            }
            #[cfg(feature = "synth")]
            {
                let (sl, sr2) = self.synth.tick(sr);
                l += sl; r += sr2;
            }
            #[cfg(feature = "audio-profile")]
            super::profiling::mark(super::profiling::Stage::Sample);
            let s = self.sample.tick(sr) * 0.8;
            l += s; r += s;
            // master bus: stereo delay, then mono-fed reverb summed to both.
            #[cfg(feature = "audio-profile")]
            super::profiling::mark(super::profiling::Stage::MasterFx);
            let (dl, dr) = self.delay.process(l, r);
            l = dl; r = dr;
            let rv = self.reverb.as_mut().map_or(0.0, |reverb| reverb.process((l + r) * 0.5));
            l += rv; r += rv;
            // default amp without hard-clipping dense chords.
            #[cfg(feature = "audio-profile")]
            let limit_start = super::profiling::cycles();
            (l, r) = limit_stereo(l, r);
            #[cfg(feature = "audio-profile")]
            { limit_cycles = limit_cycles.wrapping_add(super::profiling::cycles().wrapping_sub(limit_start)); }
            if l.abs() > out_peak { out_peak = l.abs(); }
            match nc {
                1 => frame[0] = (l + r) * 0.5,
                _ => {
                    frame[0] = l;
                    frame[1] = r;
                    for v in frame[2..].iter_mut() { *v = (l + r) * 0.5; }
                }
            }
        }
        // Per-channel C0..C3 merges the 4 sfx voices (by index) with the synth
        // patches (by pid) at the same 0.25 master gain, so simple sfx_* carts
        // (glider) and synth_* carts both light up the channel meters.
        let mut chv = [0u32; 4];
        for i in 0..4 {
            ch_peak[i] *= 0.4;
            if self.voices[i].active() { chv[i] = 1; }
        }
        #[cfg(feature = "synth")]
        {
            let sp = self.synth.take_pid_peaks();
            let sv = self.synth.pid_voices();
            for i in 0..4 { ch_peak[i] += sp[i] * 0.4; chv[i] += sv[i]; }
        }
        #[cfg(feature = "audio-profile")]
        super::profiling::set_limiter_cycles(limit_cycles);
        self.meter.set_out(out_peak);
        self.meter.set_channels(ch_peak, chv);
    }
}

fn duration_samples(ms: u32, sr: u32) -> u32 {
    (ms / 1000).saturating_mul(sr)
        .saturating_add((ms % 1000) * (sr / 1000))
        .saturating_add((ms % 1000) * (sr % 1000) / 1000)
}

#[cfg(test)]
mod duration_tests {
    use super::*;
    #[test]
    fn milliseconds_match_wide_oracle_without_runtime_wide_arithmetic() {
        let mut bits=0x12345678u32;
        for _ in 0..100000 {
            bits=bits.wrapping_mul(1664525).wrapping_add(1013904223); let ms=bits;
            bits=bits.wrapping_mul(1664525).wrapping_add(1013904223); let sr=bits;
            let expected=((ms as u64*sr as u64)/1000).min(u32::MAX as u64) as u32;
            assert_eq!(duration_samples(ms,sr),expected);
        }
    }
}

#[cfg(test)]
mod limiter_tests {
    use super::*;

    #[cfg(not(all(target_os = "none", target_pointer_width = "32")))]
    #[test]
    fn host_limiter_preserves_the_recorded_tanh_bits() {
        let mut bits = 0x12345678u32;
        for _ in 0..100_000 {
            bits = bits.wrapping_mul(1664525).wrapping_add(1013904223);
            let x = f32::from_bits(bits);
            let expected = math::tanh(x);
            let actual = limit_sample(x);
            if expected.is_nan() { assert!(actual.is_nan()); }
            else { assert_eq!(actual.to_bits(), expected.to_bits(), "x={x:?}"); }
        }
    }

    #[test]
    fn limiter_curve_stays_below_one_tenth_of_a_pcm16_step() {
        let mut previous = -1.0;
        for i in 0..=600_000 {
            let x = -3.0 + i as f32 * 0.00001;
            let value = approximate_limit_sample(x);
            assert!((value - math::tanh(x)).abs() < 0.000002);
            assert!(value >= previous);
            assert!(value.abs() <= 1.0);
            assert_eq!(approximate_limit_sample(-x).to_bits(), (-value).to_bits());
            previous = value;
        }
        for x in [-0.0, 0.0, -3.00001, -3.0, 3.0, 3.00001] {
            assert!((approximate_limit_sample(x) - math::tanh(x)).abs() < 0.000002);
        }
    }

    #[test]
    fn stereo_limiter_preserves_equal_opposite_and_special_inputs() {
        let cases = [0.0, -0.0, 0.01, -0.01, 1.0, -1.0, 100.0, f32::INFINITY, f32::NEG_INFINITY, f32::from_bits(0x7fc01234)];
        for l in cases {
            for r in cases {
                let (a, b) = limit_stereo(l, r);
                for (actual, input) in [(a, l), (b, r)] {
                    let expected = math::tanh(input * 0.8);
                    if input.is_nan() { assert!(actual.is_nan()); }
                    else { assert!((actual - expected).abs() <= 0.000002); }
                }
                if l.to_bits() == r.to_bits() { assert_eq!(a.to_bits(), b.to_bits()); }
            }
        }
    }
}

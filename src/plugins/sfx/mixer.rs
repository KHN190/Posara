use crate::sfx::output::Meter;
use crate::sfx::sample::SamplePlayer;
use crate::sfx::seq::{note_to_freq, Event, Sequencer};
use crate::sfx::voice::{Voice, WAVE_NOISE, WAVE_SQUARE};

// VM thread → audio thread commands, carried over a lock-free SPSC ring
pub enum Cmd {
    Inst(usize, u8, u32, u32, f32, u32),
    Pan(usize, f32, f32),
    Fx(usize, u8, f32, f32),
    Lfo(usize, u8, u8, f32, f32),
    Play(usize, f32, f32, u32),
    PlayMidi(usize, i64, f32, u32),
    Off(usize),
    Tone(f32, u32, f32, u32),
    Noise(u32, f32, u32),
    Wave(u8, f32, u32, f32, u32),
    Seq(Vec<Event>, u32),
    SeqStop,
    Sample(Vec<u8>, f32, f32),
    SampleStop,
    #[cfg(feature = "synth")]
    SynOsc(usize, usize, u8, i64, f32, f32),
    #[cfg(feature = "synth")]
    SynFilter(usize, u8, f32, f32),
    #[cfg(feature = "synth")]
    SynEnv(usize, usize, u8, f32, f32, f32, f32, f32),
    #[cfg(feature = "synth")]
    SynLfo(usize, u8, f32, f32),
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

pub struct Mixer {
    pub voices: [Voice; 4],
    pub seq: Sequencer,
    pub sample: SamplePlayer,
    pub sample_rate: u32,
    pub out_channels: usize,
    due: Vec<Event>,
    meter: Meter,
    #[cfg(feature = "synth")]
    synth: super::synth::Synth,
}

impl Mixer {
    pub fn new(sample_rate: u32, out_channels: usize, meter: Meter) -> Self {
        Self {
            voices: [Voice::new(); 4],
            seq: Sequencer::empty(),
            sample: SamplePlayer::default(),
            sample_rate,
            out_channels,
            due: Vec::with_capacity(4),
            meter,
            #[cfg(feature = "synth")]
            synth: super::synth::Synth::new(),
        }
    }

    pub fn play_sample(&mut self, bytes: Vec<u8>, rate: f32, vol: f32) {
        self.sample.play(bytes, rate, vol);
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
            #[cfg(feature = "synth")]
            Cmd::SynOsc(pid, idx, w, semi, fine, lvl) => self.synth.osc(pid, idx, w, semi, fine, lvl),
            #[cfg(feature = "synth")]
            Cmd::SynFilter(pid, k, c, r) => self.synth.filter(pid, k, c, r),
            #[cfg(feature = "synth")]
            Cmd::SynEnv(pid, slot, t, d, a, dec, s, r) => self.synth.env(pid, slot, t, d, a, dec, s, r),
            #[cfg(feature = "synth")]
            Cmd::SynLfo(pid, t, rate, dep) => self.synth.lfo(pid, t, rate, dep),
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
        ((ms as u64) * (self.sample_rate as u64) / 1000) as u32
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

    pub fn play_midi(&mut self, ch: usize, note: i64, vel: f32, dur_ms: u32) {
        self.play(ch, note_to_freq(note.clamp(0, 127) as u8), vel, dur_ms);
    }

    pub fn off(&mut self, ch: usize) {
        if let Some(v) = self.voices.get_mut(ch) { v.off(); }
    }

    pub fn play_seq(&mut self, events: Vec<Event>, ms_per_tick: u32) {
        let spt = (ms_per_tick as f32) * (self.sample_rate as f32) / 1000.0;
        self.seq.load(events, spt);
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
        self.due.clear();
        self.seq.advance(&mut self.due);
        let ms_per_tick = self.seq.spt * 1000.0 / self.sample_rate as f32;
        for i in 0..self.due.len() {
            let ev = self.due[i];
            self.play(ev.ch, note_to_freq(ev.note), ev.vel, (ev.dur as f32 * ms_per_tick) as u32);
        }
    }

    pub fn mix(&mut self, out: &mut [f32]) {
        let nc = self.out_channels.max(1);
        let sr = self.sample_rate as f32;
        let mut out_peak = 0.0f32;
        let mut ch_peak = [0.0f32; 4];
        for frame in out.chunks_mut(nc) {
            self.advance_seq();
            let (mut l, mut r) = (0.0f32, 0.0f32);
            for (i, v) in self.voices.iter_mut().enumerate() {
                let (vl, vr) = v.tick(sr);
                l += vl; r += vr;
                let a = vl.abs().max(vr.abs());
                if a > ch_peak[i] { ch_peak[i] = a; }
            }
            #[cfg(feature = "synth")]
            {
                let sy = self.synth.tick(sr);
                l += sy; r += sy;
            }
            let s = self.sample.tick(sr) * 0.8;
            l = (l * 0.25 + s).clamp(-1.0, 1.0);
            r = (r * 0.25 + s).clamp(-1.0, 1.0);
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
            ch_peak[i] *= 0.25;
            if self.voices[i].active() { chv[i] = 1; }
        }
        #[cfg(feature = "synth")]
        {
            let sp = self.synth.take_pid_peaks();
            let sv = self.synth.pid_voices();
            for i in 0..4 { ch_peak[i] += sp[i] * 0.25; chv[i] += sv[i]; }
        }
        self.meter.set_out(out_peak);
        self.meter.set_channels(ch_peak, chv);
    }
}

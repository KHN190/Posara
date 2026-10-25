#![cfg(feature = "synth")]

use posara::sfx::mixer::{Cmd, Retired};
use posara::sfx::seq::Event;
use posara::sfx::{AudioMeter, Mixer, DELAY_CAP_MS};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;

struct Counting;

thread_local! {
    static ON: Cell<bool> = const { Cell::new(false) };
    static HITS: Cell<usize> = const { Cell::new(0) };
    static BYTES: Cell<usize> = const { Cell::new(0) };
}

fn hit(bytes: usize) {
    let _ = ON.try_with(|on| if on.get() {
        HITS.with(|h| h.set(h.get() + 1));
        BYTES.with(|b| b.set(b.get() + bytes));
    });
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 { hit(l.size()); unsafe { System.alloc(l) } }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) { hit(0); unsafe { System.dealloc(p, l) } }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 { hit(n); unsafe { System.realloc(p, l, n) } }
}

#[global_allocator]
static ALLOC: Counting = Counting;

fn counted(f: impl FnOnce()) -> (usize, usize) {
    HITS.with(|h| h.set(0));
    BYTES.with(|b| b.set(0));
    ON.with(|o| o.set(true));
    f();
    ON.with(|o| o.set(false));
    (HITS.with(|h| h.get()), BYTES.with(|b| b.get()))
}

fn heap_hits(f: impl FnOnce()) -> usize { counted(f).0 }

const SR: u32 = 48_000;
const BLOCK: usize = 480;

fn mixer() -> Mixer {
    let mut m = Mixer::new(SR, 2, Arc::new(AudioMeter::default()), DELAY_CAP_MS);
    m.apply(Cmd::SynVoices(16));
    m
}

fn events(n: u32) -> Vec<Event> {
    (0..n).map(|i| Event { tick: i * 4, ch: 0, note: 60 + (i % 12) as u8, vel: 0.8, dur: 2 }).collect()
}

#[test]
fn live_commands_and_mixing_never_touch_the_heap() {
    let mut m = mixer();
    let mut buf = vec![0f32; BLOCK * 2];
    let past_cap = (DELAY_CAP_MS as usize * SR as usize / 1000) / BLOCK + 10;
    let cmds = [
        Cmd::SynOsc(0, 0, 1, 0, 0.0, 1.0),
        Cmd::SynOsc(0, 1, 5, 12, 0.0, 0.5),
        Cmd::SynFilter(0, 0, 2400.0, 0.3),
        Cmd::SynEnv(0, 0, 0, 100.0, 0.004, 0.2, 0.6, 0.3),
        Cmd::SynFx(0, 1, 0.4, 0.0),
        Cmd::BusDelay(250, 0.3, 0.2),
        Cmd::BusDelay(520, 0.4, 0.3),
        Cmd::BusDelay(60_000, 0.4, 0.3),
        Cmd::BusReverb(0.5, 0.3, 0.2),
        Cmd::SynOn(0, 60, 0.8, 4800),
        Cmd::Tone(440.0, 200, 0.5, 0),
    ];
    let hits = heap_hits(|| {
        for c in cmds { m.apply(c); }
        for _ in 0..past_cap { m.mix(&mut buf); }
        m.apply(Cmd::SynOff(0, 60));
        m.apply(Cmd::BusDelay(0, 0.0, 0.0));
        for _ in 0..5 { m.mix(&mut buf); }
    });
    assert_eq!(hits, 0);
}

#[test]
fn replacing_a_sequence_or_sample_frees_nothing_on_the_audio_thread() {
    let mut m = mixer();
    let bin = m.retired();
    let mut buf = vec![0f32; BLOCK * 2];
    m.apply(Cmd::Seq(events(64), 100));
    m.apply(Cmd::Sample(vec![128u8; 4096], 1.0, 1.0));
    m.mix(&mut buf);
    let (seq, smp) = (events(32), vec![100u8; 2048]);
    let hits = heap_hits(|| {
        m.apply(Cmd::Seq(seq, 100));
        m.apply(Cmd::Sample(smp, 1.0, 1.0));
        m.apply(Cmd::SeqStop);
        m.apply(Cmd::SampleStop);
        for _ in 0..5 { m.mix(&mut buf); }
    });
    assert_eq!(hits, 0);

    let mut back: Vec<(&str, usize)> = Vec::new();
    while let Some(r) = bin.try_pop() {
        back.push(match r {
            Retired::Seq(v) => ("seq", v.len()),
            Retired::Sample(v) => ("sample", v.len()),
        });
    }
    back.sort();
    assert_eq!(back, [("sample", 4096), ("seq", 64)]);
}

#[test]
fn mono_mixer_reserves_a_single_delay_line() {
    let meter = || Arc::new(AudioMeter::default());
    let (_, stereo) = counted(|| drop(Mixer::new(SR, 2, meter(), DELAY_CAP_MS)));
    let (_, mono) = counted(|| drop(Mixer::new(SR, 1, meter(), DELAY_CAP_MS)));
    let line = DELAY_CAP_MS as usize * SR as usize / 1000 * std::mem::size_of::<f32>();
    assert!(stereo >= 2 * line, "stereo reserves {stereo} bytes, want two lines of {line}");
    assert!(stereo - mono >= line, "mono reserves {mono} bytes, want one line less than {stereo}");
}

#[test]
fn many_events_on_one_tick_allocate_nothing() {
    let mut m = mixer();
    let mut buf = vec![0f32; BLOCK * 2];
    let chord: Vec<Event> = (0..8).map(|i| Event { tick: 0, ch: i % 4, note: 60 + i as u8, vel: 0.8, dur: 4 }).collect();
    let hits = heap_hits(|| {
        m.apply(Cmd::Seq(chord, 100));
        for _ in 0..5 { m.mix(&mut buf); }
    });
    assert_eq!(hits, 0);
}

#[test]
fn loading_a_long_sequence_never_sorts_on_the_audio_thread() {
    let mut m = mixer();
    let mut buf = vec![0f32; BLOCK * 2];
    let shuffled: Vec<Event> = (0..1024u32).map(|i| Event { tick: (i * 37 % 1024) * 4, ch: 0, note: 60, vel: 0.8, dur: 2 }).collect();
    let hits = heap_hits(|| {
        m.apply(Cmd::Seq(shuffled, 100));
        m.mix(&mut buf);
    });
    assert_eq!(hits, 0);
}


mod sfx_ops;

mod random_streams {
    use super::sfx_ops::session;
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn any_command_stream_mixes_without_heap_ops(ops in session(), seq in proptest::collection::vec((0u32..32, 0u8..128), 0..64)) {
            let mut m = mixer();
            let mut cmds: Vec<_> = ops.iter().map(|(o, n)| (o.cmd(), *n)).collect();
            let events: Vec<Event> = seq.iter().map(|&(tick, note)| Event { tick, ch: (note % 4) as usize, note, vel: 0.7, dur: 2 }).collect();
            let seq_cmd = Cmd::Seq(posara::sfx::seq::by_tick(events), 5);
            let sample = Cmd::Sample(vec![128u8; 4800], 22_050.0, 0.5);
            let mut buf = vec![0f32; BLOCK * 2];
            let hits = heap_hits(|| {
                m.apply(seq_cmd);
                m.apply(sample);
                for (c, n) in cmds.drain(..) {
                    m.apply(c);
                    for _ in 0..n { m.mix(&mut buf); }
                }
            });
            prop_assert_eq!(hits, 0);
        }
    }
}

#[test]
fn disabled_reverb_saves_memory_without_changing_dry_or_delay_audio() {
    let mut full = None;
    let mut lean = None;
    let (_, full_bytes) = counted(|| full = Some(Mixer::new(SR, 1, Arc::new(AudioMeter::default()), DELAY_CAP_MS)));
    let (_, lean_bytes) = counted(|| lean = Some(Mixer::new_with_reverb(SR, 1, Arc::new(AudioMeter::default()), DELAY_CAP_MS, false)));
    assert!(full_bytes - lean_bytes > 24 * 1024);
    let (mut full, mut lean) = (full.unwrap(), lean.unwrap());
    for m in [&mut full, &mut lean] {
        m.apply(Cmd::BusDelay(125, 0.3, 0.2));
        m.apply(Cmd::SynOn(0, 60, 0.7, 4800));
    }
    let mut a = [0.0f32; BLOCK];
    let mut b = [0.0f32; BLOCK];
    assert_eq!(heap_hits(|| {
        for _ in 0..100 {
            full.mix(&mut a);
            lean.mix(&mut b);
            for (x, y) in a.iter().zip(b.iter()) { assert_eq!(x.to_bits(), y.to_bits()); }
        }
    }), 0);
}

#[test]
fn prepared_delay_matches_eager_output_and_swaps_without_heap_ops() {
    use posara_core::sfx::bus::Delay;
    let mut eager = Mixer::new_with_reverb(SR, 1, Arc::new(AudioMeter::default()), 750, false);
    let mut lean = Mixer::new_with_reverb(SR, 1, Arc::new(AudioMeter::default()), 0, false);
    let mut prepared = Delay::try_new(36_000, false).unwrap();
    assert_eq!(heap_hits(|| lean.swap_delay(&mut prepared)), 0);
    for m in [&mut eager, &mut lean] {
        m.apply(Cmd::BusDelay(125, 0.3, 0.4));
        m.apply(Cmd::SynOn(0, 60, 0.7, 90));
    }
    let (mut a, mut b) = ([0.0; BLOCK], [0.0; BLOCK]);
    assert_eq!(heap_hits(|| {
        for _ in 0..100 {
            eager.mix(&mut a); lean.mix(&mut b);
            for (x,y) in a.iter().zip(&b) { assert_eq!(x.to_bits(),y.to_bits()); }
        }
    }), 0);
    let mut empty = Delay::try_new(0, false).unwrap();
    assert_eq!(heap_hits(|| lean.swap_delay(&mut empty)), 0);
    assert_eq!(heap_hits(|| {
        lean.apply(Cmd::BusDelay(750, 0.5, 1.0));
        lean.mix(&mut b);
    }), 0);
}

#[test]
fn absent_delay_releases_its_entire_line_without_changing_dry_pcm() {
    let mut full = None; let mut lean = None;
    let (_, full_bytes) = counted(|| full = Some(Mixer::new_with_reverb(SR, 1, Arc::new(AudioMeter::default()), 750, false)));
    let (_, lean_bytes) = counted(|| lean = Some(Mixer::new_with_reverb(SR, 1, Arc::new(AudioMeter::default()), 0, false)));
    assert_eq!(full_bytes - lean_bytes, 144_000);
    let (mut full,mut lean)=(full.unwrap(),lean.unwrap());
    for m in [&mut full,&mut lean] { m.apply(Cmd::SynOn(0,60,0.7,90)); }
    let (mut a,mut b)=([0.0;BLOCK],[0.0;BLOCK]);
    for _ in 0..20 {
        full.mix(&mut a);lean.mix(&mut b);
        assert_eq!(a,b);
    }
}

#[test]
fn impossible_delay_capacity_returns_an_error() {
    assert!(posara_core::sfx::bus::Delay::try_new(usize::MAX, true).is_err());
}

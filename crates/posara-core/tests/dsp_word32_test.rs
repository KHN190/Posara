#![cfg(feature = "sfx")]
use posara_core::sfx::{sample::{Fixed, Pos}, seq::{self, Event, Sequencer}};
fn event(tick: u32) -> Event { Event { tick, ch: 0, note: 60, vel: 1.0, dur: 1 } }
#[test]
fn sequence_clock_crosses_float_integer_limit() {
    let mut s = Sequencer::empty();
    s.load(vec![event(1)], 16_777_218.0);
    for _ in 0..16_777_217 { s.tick(); }
    assert!(s.next_due().is_none());
    s.tick(); assert!(s.next_due().is_some());
}
#[test]
fn adjacent_deadlines_preserve_fraction_sparse_and_same_tick() {
    for spt in [1.0, 1.1, 1.25, 1.5, 2.25, 3.125, 17.1] {
        let mut s = Sequencer::empty();
        s.load(vec![event(0), event(1), event(1), event(1000)], spt);
        let mut seen = Vec::new();
        for frame in 1..=20000 { s.tick(); while let Some(e) = s.next_due() { seen.push((e.tick, frame)); } }
        let deadline = |tick| ((tick as f64 * spt as f64).ceil() as i32).max(1);
        assert_eq!(seen, vec![(0,1),(1,deadline(1)),(1,deadline(1)),(1000,deadline(1000))]);
        s.load(vec![event(2)], 1.5); s.tick(); assert!(s.next_due().is_none());
        s.tick(); assert!(s.next_due().is_none()); s.tick(); assert!(s.next_due().is_some());
    }
}
#[test]
fn sample_position_carries_without_wrapping_at_end() {
    let mut p = Fixed::default(); p.advance(16_777_216.0);
    for _ in 0..4 {p.advance(0.25);} assert_eq!(p.bit(), 16_777_217);
    p.advance(f32::MAX); assert_eq!(p.bit(), usize::MAX);
    p.advance(1.0); assert_eq!(p.bit(), usize::MAX);
}
#[test]
fn event_bytes_preserve_cross_word_fields_and_reserved_nonzero() {
    let bytes = seq::pack_bytes(65535,7,255,127,65535);
    let e = seq::unpack_bytes(bytes);
    assert_eq!((e.tick,e.ch,e.note,e.dur),(65535,7,255,65535)); assert_eq!(e.vel,1.27);
    assert_eq!(seq::decode(&[0,0,0,0,0,0,0,128]).len(),1);
}
#[test]
fn integer_sqrt_accepts_both_widths_at_maximum() {
    assert_eq!(posara_core::math::isqrt(i32::MAX),46340);
    assert_eq!(posara_core::math::isqrt_ceil(i32::MAX),46341);
    assert_eq!(posara_core::math::isqrt(i64::MAX),3037000499);
}

#[test]
fn nonfinite_and_oversized_tempo_never_panics_or_fires_early() {
    for tempo in [f32::INFINITY, f32::MAX, u32::MAX as f32] {
        let mut s = Sequencer::empty(); s.load(vec![event(1)], tempo);
        for _ in 0..3 { s.tick(); assert!(s.next_due().is_none()); }
    }
    for tempo in [f32::NAN, f32::NEG_INFINITY, -1.0] {
        let mut s = Sequencer::empty(); s.load(vec![event(1)], tempo);
        s.tick(); assert!(s.next_due().is_some());
    }
    let mut same = vec![event(2),event(1),event(2)]; same[0].note=61; same[2].note=62;
    let sorted = seq::by_tick(same); assert_eq!(sorted.iter().map(|e|e.note).collect::<Vec<_>>(),vec![60,61,62]);
}

#[test]
fn default_sample_position_is_fixed_on_the_host_too() {
    let default: posara_core::sfx::sample::SamplePlayer = Default::default();
    let mut fixed: posara_core::sfx::sample::SamplePlayer<Fixed> = default;
    fixed.play(vec![255], 48000.0, 1.0);
    for _ in 0..8 { assert!(fixed.tick(48000.0) > 0.0); }
    assert_eq!(fixed.tick(48000.0), 0.0);
}
#[test]
fn integer_sqrt_small_negative_and_large_boundaries() {
    for n in [i32::MIN,-1,0,1,2,3,4,5,2147395600,i32::MAX] {
        let s = posara_core::math::isqrt(n); let c=posara_core::math::isqrt_ceil(n);
        let n=n.max(0) as i64; assert!(s as i64*s as i64<=n); assert!((s as i64+1)*(s as i64+1)>n);
        assert!(c as i64*c as i64>=n); assert!(c==0 || (c as i64-1)*(c as i64-1)<n);
    }
    assert_eq!(posara_core::math::isqrt(i64::MIN),0);
    assert_eq!(posara_core::math::isqrt_ceil(i64::MAX),3037000500);
}

#[test]
fn tempo_changes_at_next_boundary_without_retiming_elapsed_music() {
    let mut s=Sequencer::empty(); s.load(vec![event(1),event(2)],3.0);
    s.tick(); s.spt=1.5;
    s.tick(); assert!(s.next_due().is_none());
    s.tick(); assert_eq!(s.next_due().unwrap().tick,1);
    s.tick(); assert!(s.next_due().is_none());
    s.tick(); assert_eq!(s.next_due().unwrap().tick,2);
}

use posara_core::audio::to_mono_i16;

#[test]
fn averages_left_right() {
    let mut out = [0i16; 3];
    to_mono_i16(&[1.0, 1.0, 1.0, -1.0, -0.5, -0.5], &mut out);
    assert_eq!(out, [32767, 0, -16383]);
}

#[test]
fn clamps_out_of_range() {
    let mut out = [0i16; 2];
    to_mono_i16(&[2.0, 2.0, -3.0, -3.0], &mut out);
    assert_eq!(out, [32767, -32767]);
}

#[test]
fn writes_min_of_both_lengths() {
    let mut out = [7i16; 3];
    to_mono_i16(&[0.0, 0.0], &mut out);
    assert_eq!(out, [0, 7, 7]);
}

#[test]
fn mono_sample_scales_and_clamps() {
    use posara_core::audio::to_i16;
    assert_eq!([to_i16(0.5), to_i16(-1.5), to_i16(1.0)], [16383, -32767, 32767]);
}

#[test]
fn widened_right_channel_lags_by_the_width() {
    use posara_core::audio::Width;
    let mut w = Width::new(2);
    w.set(true);
    let out: Vec<_> = [1.0, 2.0, 3.0, 4.0].iter().map(|&x| w.next(x)).collect();
    assert_eq!(out, [(1.0, 0.0), (2.0, 0.0), (3.0, 1.0), (4.0, 2.0)]);
}

#[test]
fn narrow_output_is_the_same_on_both_sides() {
    use posara_core::audio::Width;
    let mut w = Width::new(2);
    assert_eq!([w.next(1.0), w.next(-0.5)], [(1.0, 1.0), (-0.5, -0.5)]);
}

#[test]
fn widening_mid_stream_lags_from_the_signal_already_heard() {
    use posara_core::audio::Width;
    let mut w = Width::new(1);
    w.next(1.0);
    w.set(true);
    assert_eq!(w.next(2.0), (2.0, 1.0));
}

use posara_core::audio::{to_i16, Width};
use proptest::prelude::*;

proptest! {
    #[test]
    fn samples_quantize_within_one_step_and_stay_in_range(x in -4.0f32..4.0) {
        let q = to_i16(x) as f32 / 32767.0;
        prop_assert!(q.abs() <= 1.0);
        prop_assert!((q - x.clamp(-1.0, 1.0)).abs() < 1.0 / 32767.0);
    }

    #[test]
    fn quantizing_keeps_order_and_sign(a in -2.0f32..2.0, b in -2.0f32..2.0) {
        if a <= b { prop_assert!(to_i16(a) <= to_i16(b)); }
        prop_assert_eq!(to_i16(-a), -to_i16(a));
    }

    #[test]
    fn mono_is_the_quantized_mean_of_each_pair(lr in prop::collection::vec((-1.5f32..1.5, -1.5f32..1.5), 0..64)) {
        let flat: Vec<f32> = lr.iter().flat_map(|&(l, r)| [l, r]).collect();
        let mut out = vec![0i16; lr.len()];
        to_mono_i16(&flat, &mut out);
        for (o, &(l, r)) in out.iter().zip(&lr) {
            let mean = ((l + r) / 2.0).clamp(-1.0, 1.0);
            prop_assert!((*o as f32 / 32767.0 - mean).abs() < 1.0 / 32767.0 + 1e-6);
        }
    }

    #[test]
    fn width_matches_a_delay_line_model(lag in 1usize..64, input in prop::collection::vec((-1.0f32..1.0, any::<bool>()), 0..400)) {
        let mut w = Width::new(lag);
        let mut heard: Vec<f32> = Vec::new();
        for (x, on) in input {
            w.set(on);
            let want_r = if on { if heard.len() >= lag { heard[heard.len() - lag] } else { 0.0 } } else { x };
            prop_assert_eq!(w.next(x), (x, want_r));
            heard.push(x);
        }
    }
}

#[test]
fn silence_is_what_a_broken_sample_becomes() {
    assert_eq!(to_i16(f32::NAN), 0);
    assert_eq!([to_i16(f32::INFINITY), to_i16(f32::NEG_INFINITY)], [32767, -32767]);
}

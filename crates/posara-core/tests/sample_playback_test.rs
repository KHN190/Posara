#![cfg(feature = "sfx")]

use posara_core::sfx::sample::{Fixed, Pos, SamplePlayer};
use proptest::prelude::*;

fn silent_after<P: Pos>(rate: f32, sr: f32, bytes: usize) -> usize {
    let mut p = SamplePlayer::<P>::default();
    p.play(vec![0xFF; bytes], rate, 1.0);
    (0..100_000).take_while(|_| p.tick(sr) != 0.0).count()
}

fn samples_until_silent(rate: f32, sr: f32, bytes: usize) -> usize {
    let n = silent_after::<f64>(rate, sr, bytes);
    assert_eq!(silent_after::<Fixed>(rate, sr, bytes), n, "device fixed point disagrees");
    n
}

#[test]
fn duration_scales_with_rate_and_stream_length() {
    for (rate, sr, bytes, want) in [
        (48_000.0, 48_000.0, 4, 32),
        (24_000.0, 48_000.0, 4, 64),
        (96_000.0, 48_000.0, 1000, 4000),
    ] {
        assert_eq!(samples_until_silent(rate, sr, bytes), want);
    }
}

proptest! {
    #[test]
    fn device_fixed_point_plays_what_the_float_path_plays(
        bytes in prop::collection::vec(any::<u8>(), 1..64),
        rate in 100.0f32..96_000.0, vol in 0.0f32..1.0,
        sr in prop::sample::select(vec![22_050.0f32, 44_100.0, 48_000.0]),
    ) {
        let mut host = SamplePlayer::<f64>::default();
        let mut device = SamplePlayer::<Fixed>::default();
        host.play(bytes.clone(), rate, vol);
        device.play(bytes, rate, vol);
        for n in 0..200_000 {
            let (h, d) = (host.tick(sr), device.tick(sr));
            prop_assert!((h - d).abs() <= 1e-4, "sample {n}: host {h} device {d}");
            if h == 0.0 && d == 0.0 && n > 0 { break; }
        }
    }
}

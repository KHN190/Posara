#![cfg(feature = "sfx")]

use posara_core::sfx::sample::SamplePlayer;

fn samples_until_silent(rate: f32, sr: f32, bytes: usize) -> usize {
    let mut p = SamplePlayer::default();
    p.play(vec![0xFF; bytes], rate, 1.0);
    (0..100_000).take_while(|_| p.tick(sr) != 0.0).count()
}

#[test]
fn each_bit_plays_once_at_the_output_rate() {
    assert_eq!(samples_until_silent(48_000.0, 48_000.0, 4), 32);
}

#[test]
fn half_rate_holds_each_bit_for_two_samples() {
    assert_eq!(samples_until_silent(24_000.0, 48_000.0, 4), 64);
}

#[test]
fn long_stream_ends_on_the_last_bit() {
    assert_eq!(samples_until_silent(96_000.0, 48_000.0, 1000), 4000);
}

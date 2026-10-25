#![cfg(all(feature = "synth", feature = "std"))]

use posara_core::sfx::{fx::{Fx, FX_LOPASS}, synth::Synth};

#[test]
fn lowpass_kind_zero_keeps_its_unoptimized_pcm() {
    let mut synth = Synth::new();
    synth.set_voices(1);
    synth.osc(0, 0, 1, 0, 0.0, 1.0);
    synth.osc(0, 1, 1, 0, 0.0, 0.0);
    synth.filter(0, 0, 400.0, 0.0);
    synth.env(0, 0, 0, 100.0, 0.0, 0.0, 1.0, 10.0);
    synth.note_on(0, 69, 1.0, 0);
    assert_eq!(synth.tick(48000.0).0.to_bits(), 0);
}

#[test]
fn filter_cache_computes_its_first_coefficient_even_for_zero_keys() {
    let mut fx = Fx::default();
    fx.kind = FX_LOPASS;
    fx.amt = 1.0;
    assert_eq!(fx.process(1.0, 0.0).to_bits(), 1.0f32.to_bits());
}

#[test]
fn filter_cache_distinguishes_positive_and_negative_zero_sample_rates() {
    let mut fx = Fx::default();
    fx.kind = FX_LOPASS;
    fx.amt = 1.0;
    fx.process(0.0, 48000.0);
    assert_eq!(fx.process(1.0, 0.0).to_bits(), 1.0f32.to_bits());
    assert_eq!(fx.process(0.0, -0.0).to_bits(), 1.0f32.to_bits());
}

#[test]
fn disabled_effects_are_bit_exact_passthrough() {
    for kind in 1..=5 {
        let mut fx = Fx::default();
        fx.set(kind, 0.0, 800.0);
        for bits in [0u32, 0x80000000, 0x3f000000, 0xbf000000, 0x7f800000, 0x7fc01234] {
            let x = f32::from_bits(bits);
            assert_eq!(fx.process(x, 48000.0).to_bits(), bits, "kind={kind}");
        }
    }
}

#[test]
fn reenabled_bitcrush_does_not_replay_an_old_held_sample() {
    let mut fx = Fx::default();
    fx.set(1, 0.8, 0.0);
    fx.process(0.9, 48000.0);
    fx.amt = 0.0;
    fx.process(0.0, 48000.0);
    fx.amt = 0.8;
    let mut fresh = Fx::default();
    fresh.set(1, 0.8, 0.0);
    assert_eq!(fx.process(-0.5, 48000.0).to_bits(), fresh.process(-0.5, 48000.0).to_bits());
}

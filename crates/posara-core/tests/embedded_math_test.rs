#![cfg(feature = "sfx")]
use posara_core::math::{sin, cos};
#[test]
fn trig_matches_reference_across_audio_phase_range() {
    for i in -131072..=131072 {
        let x = i as f32 / 2048.0;
        assert!((sin(x) - libm::sinf(x)).abs() < 0.000012, "sin {x}");
        assert!((cos(x) - libm::cosf(x)).abs() < 0.000012, "cos {x}");
        assert!((sin(x)*sin(x) + cos(x)*cos(x) - 1.0).abs() < 0.00002);
    }
}
#[test]
fn trig_preserves_special_values_and_large_argument_reduction() {
    assert_eq!(sin(-0.0).to_bits(), (-0.0f32).to_bits());
    for x in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(sin(x).is_nan()); assert!(cos(x).is_nan());
    }
    for x in [-1e30, -1000.0, 1000.0, 1e30] {
        assert!(sin(x).to_bits().abs_diff(libm::sinf(x).to_bits()) <= 3);
        assert!(cos(x).to_bits().abs_diff(libm::cosf(x).to_bits()) <= 3);
    }
}

#[test]
fn pow_preserves_the_mature_float_algorithm() {
    let mut bits = 0xdeadbeefu32;
    for _ in 0..1000000 {
        bits = bits.wrapping_mul(1664525).wrapping_add(1013904223);
        let x = f32::from_bits(bits);
        bits = bits.wrapping_mul(1664525).wrapping_add(1013904223);
        let y = f32::from_bits(bits);
        let expected = libm::powf(x,y); let actual = posara_core::math::powf(x,y);
        assert!(actual.to_bits() == expected.to_bits() || (actual.is_nan() && expected.is_nan()), "{x} ** {y}");
    }
    for x in [0.0, -0.0, f32::MIN_POSITIVE, f32::from_bits(1), f32::MAX, f32::INFINITY, f32::NAN] {
        let expected = libm::powf(x,0.5); let actual=posara_core::math::powf(x,0.5);
        assert!(actual.to_bits()==expected.to_bits() || (actual.is_nan()&&expected.is_nan()));
    }
}

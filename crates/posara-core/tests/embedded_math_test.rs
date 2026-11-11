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


#[test]
fn oscillator_phase_kernel_preserves_amplitude_symmetry_and_wrap_continuity() {
    use posara_core::math::phase_sin;
    for i in 0..=262144 {
        let phase = i as f32 / 262144.0;
        let actual = phase_sin(phase);
        let reference = libm::sinf(phase * core::f32::consts::TAU);
        assert!((actual - reference).abs() < 0.000001, "phase={phase}");
        assert!((actual + phase_sin(1.0 - phase)).abs() < 0.000001);
        assert!(actual.abs() <= 1.000001);
    }
    for boundary in [0.0f32, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875, 1.0] {
        for offset in [-2.0f32, -1.0, 0.0, 1.0, 2.0] {
            let phase = (boundary + offset * f32::EPSILON).clamp(0.0, 1.0);
            assert!((phase_sin(phase) - libm::sinf(phase * core::f32::consts::TAU)).abs() < 0.000001);
        }
    }
}

#[test]
fn oscillator_interpolation_preserves_fm_across_notes_and_depths() {
    use posara_core::math::phase_sin;
    for note in 0..=127 {
        let step = 440.0 * libm::powf(2.0, (note as f32 - 69.0) / 12.0) / 48000.0;
        for depth in [0.0, 0.1, 0.5, 0.9, 1.0] {
            let mut phase = 0.0f32;
            for _ in 0..2048 {
                phase = (phase + step).fract();
                let mod_phase = (phase * 2.0).fract();
                let actual_phase = (phase + phase_sin(mod_phase) * depth
                    * core::f32::consts::FRAC_1_PI).rem_euclid(1.0);
                let reference_phase = (phase + libm::sinf(mod_phase * core::f32::consts::TAU)
                    * depth * core::f32::consts::FRAC_1_PI).rem_euclid(1.0);
                let error = (phase_sin(actual_phase)
                    - libm::sinf(reference_phase * core::f32::consts::TAU)).abs();
                assert!(error < 0.000003, "note={note} depth={depth} error={error}");
            }
        }
    }
}

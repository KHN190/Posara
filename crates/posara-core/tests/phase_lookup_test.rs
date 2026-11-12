use posara_core::math::phase_sin_lut;

#[test]
fn sine_lookup_preserves_cycle_shape_and_error_budget() {
    for (phase, expected) in [(0.0, 0.0), (0.25, 1.0), (0.5, 0.0), (0.75, -1.0), (1.0, 0.0)] {
        assert_eq!(phase_sin_lut(phase), expected);
    }
    let mut sum = 0.0;
    for i in 0..=65536 {
        let phase = i as f32 / 65536.0;
        let actual = phase_sin_lut(phase);
        let expected = (phase as f64 * std::f64::consts::TAU).sin();
        let error = actual as f64 - expected;
        assert!(actual.abs() <= 1.0);
        assert!(error.abs() < 8e-5, "phase={phase}, error={error}");
        sum += error * error;
        if phase <= 0.5 {
            assert!((actual + phase_sin_lut(phase + 0.5)).abs() < 2e-7);
        }
    }
    assert!((sum / 65537.0).sqrt() < 4e-5);
    for quadrant in 0..4 {
        let ascending = quadrant == 0 || quadrant == 3;
        let mut previous = phase_sin_lut(quadrant as f32 * 0.25);
        for i in 1..=4096 {
            let current = phase_sin_lut(quadrant as f32 * 0.25 + i as f32 / 16384.0);
            assert!(if ascending { current >= previous } else { current <= previous });
            previous = current;
        }
    }
}

#[test]
fn nested_fm_stays_within_propagated_error_budget() {
    for depth in [0.0f32, 0.1, 0.3, 0.5, 0.9, 1.0] {
        for carrier in 0..256 {
            for modulator in 0..256 {
                let cp = carrier as f32 / 256.0;
                let mp = (modulator as f32 + 0.375) / 256.0;
                let phase = (cp + phase_sin_lut(mp) * depth / std::f32::consts::PI).rem_euclid(1.0);
                let actual = phase_sin_lut(phase) as f64;
                let ideal_phase = cp as f64 + (mp as f64 * std::f64::consts::TAU).sin() * depth as f64 / std::f64::consts::PI;
                let ideal = (ideal_phase * std::f64::consts::TAU).sin();
                assert!((actual - ideal).abs() < 8e-5 * (1.0 + 2.0 * depth as f64) + 1e-6,
                    "depth={depth}, carrier={carrier}, modulator={modulator}");
            }
        }
    }
}

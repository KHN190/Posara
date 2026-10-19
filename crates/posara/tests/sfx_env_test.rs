#![cfg(feature = "sfx")]

use posara::sfx::env::Adsr;

fn run(a: &mut Adsr, steps: usize, dt: f32) -> f32 {
    let mut last = 0.0;
    for _ in 0..steps {
        last = a.tick(dt);
        assert!((0.0..=1.0).contains(&last), "level out of range: {last}");
    }
    last
}

#[test]
fn idle_until_gated() {
    let mut a = Adsr::default();
    assert!(!a.active());
    assert_eq!(a.tick(0.01), 0.0);
}

#[test]
fn attack_then_decay_settles_at_sustain() {
    let mut a = Adsr::default();
    a.set(0.1, 0.1, 0.4, 0.1);
    a.gate_on();
    assert!(a.active());
    let mut peak = 0.0f32;
    for _ in 0..40 { peak = peak.max(a.tick(0.005)); }
    assert!(peak > 0.9, "attack did not reach near 1.0: {peak}");
    let settled = run(&mut a, 200, 0.005);
    assert!((settled - 0.4).abs() < 0.02, "sustain off: {settled}");
}

#[test]
fn zero_attack_jumps_to_one() {
    let mut a = Adsr::default();
    a.set(0.0, 0.0, 0.5, 0.1);
    a.gate_on();
    assert_eq!(a.tick(0.01), 1.0);
}

#[test]
fn gate_off_releases_to_silence() {
    let mut a = Adsr::default();
    a.set(0.0, 0.0, 0.8, 0.05);
    a.gate_on();
    run(&mut a, 5, 0.01);
    a.gate_off();
    run(&mut a, 20, 0.01);
    assert_eq!(a.tick(0.01), 0.0);
    assert!(!a.active());
}

#[test]
fn sustain_clamped_to_unit() {
    let mut a = Adsr::default();
    a.set(0.0, 0.0, 5.0, 0.0); // out-of-range sustain
    a.gate_on();
    assert_eq!(a.tick(0.01), 1.0);
}

mod stages {
    use posara::sfx::env::Adsr;
    use proptest::prelude::*;

    const DT: f32 = 1.0 / 48_000.0;

    proptest! {
        #[test]
        fn the_envelope_rises_settles_and_releases_on_time(
            atk in 0.0f32..0.1, dec in 0.0f32..0.1, sus in 0.0f32..1.0, rel in 0.0f32..0.1, gate in 1usize..15_000,
        ) {
            let mut a = Adsr::default();
            a.set(atk, dec, sus, rel);
            a.gate_on();
            let n = |s: f32| (s / DT).ceil() as usize;
            let mut prev = 0.0f32;
            let mut held = 0.0f32;
            for i in 0..gate {
                let l = a.tick(DT);
                prop_assert!((0.0..=1.0).contains(&l));
                if i + 1 < n(atk) { prop_assert!(l >= prev - 1e-6, "attack fell at {i}"); }
                if i > n(atk) + n(dec) + 2 { prop_assert!((l - sus).abs() <= 1e-4, "sustain {l} vs {sus}"); }
                prev = l;
                held = l;
            }
            a.gate_off();
            let mut first = true;
            for i in 0..n(rel) + 3 {
                let l = a.tick(DT);
                if first { prop_assert!(l <= held + 1e-6); first = false; }
                prop_assert!(l <= prev + 1e-6 || i == 0, "release rose at {i}");
                prev = l;
            }
            prop_assert!(!a.active() && a.tick(DT) == 0.0, "still active after release");
        }
    }
}

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

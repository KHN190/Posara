#![cfg(feature = "sfx")]

mod stages {
    use posara::sfx::env::Adsr;
    use proptest::prelude::*;

    const DT: f32 = 1.0 / 48_000.0;

    proptest! {
        #[test]
        fn the_envelope_rises_settles_and_releases_on_time(
            atk in prop_oneof![Just(0.0f32), 0.0f32..0.1],
            dec in prop_oneof![Just(0.0f32), 0.0f32..0.1],
            sus in prop_oneof![0.0f32..1.0, 1.0f32..5.0],
            rel in prop_oneof![Just(0.0f32), 0.0f32..0.1],
            gate in 1usize..15_000,
        ) {
            let mut a = Adsr::default();
            prop_assert!(!a.active() && a.tick(DT) == 0.0);
            a.set(atk, dec, sus, rel);
            let sus = sus.clamp(0.0, 1.0);
            a.gate_on();
            prop_assert!(a.active());
            let n = |s: f32| (s / DT).ceil() as usize;
            let mut prev = 0.0f32;
            let mut held = 0.0f32;
            for i in 0..gate {
                let l = a.tick(DT);
                prop_assert!((0.0..=1.0).contains(&l));
                if i == 0 && atk == 0.0 { prop_assert_eq!(l, 1.0); }
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

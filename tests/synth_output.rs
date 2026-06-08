#![cfg(feature = "synth")]

use posara::sfx::synth::Synth;

#[test]
fn patches_stay_finite_and_bounded() {
    let cfgs: [(u8, i64, f32, f32, u8, f32, f32); 4] = [
        // wave, semi, fine, level, filt_kind, cutoff, env-cutoff-depth (or pitch)
        (1, 0, 0.0, 1.0, 0, 1400.0, 28.0),  // pid0 kick (slot1=pitch)
        (3, 0, 0.0, 1.0, 0, 380.0, 70.0),   // pid1 bass
        (1, 0, 0.0, 1.0, 0, 3200.0, 40.0),  // pid2 mid
        (2, 0, 0.0, 0.9, 0, 5200.0, 50.0),  // pid3 high
    ];
    for (pid, &(wave, semi, fine, level, fk, cut, depth)) in cfgs.iter().enumerate() {
        for &sr in &[44100.0f32, 48000.0] {
            let mut s = Synth::new();
            s.set_voices(16);
            s.osc(pid, 0, wave, semi, fine, level);
            s.filter(pid, fk, cut, 60.0);
            s.env(pid, 0, 0, 100.0, 0.004, 0.42, 0.3, 0.32);
            let tgt = if pid == 0 { 2 } else { 1 };
            s.env(pid, 1, tgt, depth, 0.002, 0.26, 0.0, 0.2);
            s.note_on(pid, [36, 44, 63, 75][pid], 0.8, (sr as u32) / 2);

            let mut peak = 0.0f32;
            for _ in 0..(sr as usize / 2) {
                let (vl, vr) = s.tick(sr);
                let v = vl + vr;
                assert!(v.is_finite(), "pid {pid} @ {sr}Hz produced non-finite sample");
                assert!(v.abs() < 8.0, "pid {pid} @ {sr}Hz blew up: {v}");
                peak = peak.max(v.abs());
            }
            assert!(peak > 0.01, "pid {pid} @ {sr}Hz silent: {peak}");
        }
    }
}

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

const FM_SR: f32 = 48_000.0;

fn fm_render(mod_wave: u8, mod_level: f32) -> Vec<f32> {
    let mut s = Synth::new();
    s.set_voices(1);
    s.osc(0, 0, 1, 0, 0.0, 1.0);
    s.osc(0, 1, mod_wave, 12, 0.0, mod_level);
    s.note_on(0, 69, 1.0, FM_SR as u32);
    (0..9_600).map(|_| { let (l, r) = s.tick(FM_SR); l + r }).skip(480).collect()
}

fn power_at(x: &[f32], hz: f32) -> f32 {
    let c = 2.0 * (std::f32::consts::TAU * hz / FM_SR).cos();
    let (mut s1, mut s2) = (0.0f32, 0.0f32);
    for &v in x {
        let s0 = v + c * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    s1 * s1 + s2 * s2 - c * s1 * s2
}

#[test]
fn fm_at_zero_depth_is_the_plain_carrier() {
    assert_eq!(fm_render(5, 0.0), fm_render(3, 0.0));
}

#[test]
fn fm_modulator_shapes_the_carrier_instead_of_sounding() {
    let x = fm_render(5, 0.5);
    let carrier = power_at(&x, 440.0);
    assert!(power_at(&x, 1320.0) > carrier * 1e-2, "no 440+880 sideband");
    assert!(power_at(&x, 880.0) < carrier * 1e-3, "modulator heard as its own tone");
}

mod delay_bus {
    use posara::sfx::mixer::Cmd;
    use posara::sfx::{AudioMeter, Mixer, DELAY_CAP_MS};
    use std::sync::Arc;

    const BLOCK: usize = 480;

    fn run(resend: Option<(u32, f32, f32)>) -> Vec<f32> {
        let mut m = Mixer::new(48_000, 2, Arc::new(AudioMeter::default()), DELAY_CAP_MS);
        m.apply(Cmd::BusDelay(100, 0.5, 0.5));
        m.apply(Cmd::Tone(440.0, 20, 0.8, 0));
        let mut buf = vec![0f32; BLOCK * 2];
        let mut out = Vec::new();
        for i in 0..40 {
            if let (5, Some((t, f, x))) = (i, resend) { m.apply(Cmd::BusDelay(t, f, x)); }
            m.mix(&mut buf);
            out.extend_from_slice(&buf);
        }
        out
    }

    fn echo_energy(x: &[f32]) -> f32 {
        x[2 * BLOCK * 10..2 * BLOCK * 30].iter().map(|v| v * v).sum()
    }

    #[test]
    fn resending_same_delay_settings_changes_nothing() {
        assert_eq!(run(None), run(Some((100, 0.5, 0.5))));
    }

    #[test]
    fn changing_only_mix_keeps_the_echo_tail() {
        let base = echo_energy(&run(None));
        let louder = echo_energy(&run(Some((100, 0.5, 0.8))));
        assert!(base > 0.0, "no echo at all");
        assert!(louder > base, "tail cut: {louder} vs {base}");
    }

    #[test]
    fn changing_delay_time_starts_a_fresh_line() {
        let moved = echo_energy(&run(Some((150, 0.5, 0.5))));
        let base = echo_energy(&run(None));
        assert!(moved < base, "old echo survived a time change: {moved} vs {base}");
    }
}

mod sequence {
    use posara::sfx::seq::{by_tick, Event};

    fn ev(tick: u32, note: u8) -> Event { Event { tick, ch: 0, note, vel: 0.8, dur: 1 } }

    #[test]
    fn events_are_ordered_by_tick() {
        let ticks: Vec<u32> = by_tick(vec![ev(8, 60), ev(0, 61), ev(4, 62)]).iter().map(|e| e.tick).collect();
        assert_eq!(ticks, [0, 4, 8]);
    }

    #[test]
    fn events_on_one_tick_keep_submission_order() {
        let input: Vec<Event> = (0..32u8).map(|i| ev(if i % 2 == 0 { 4 } else { 0 }, i)).collect();
        let notes: Vec<u8> = by_tick(input).iter().map(|e| e.note).collect();
        let want: Vec<u8> = (0..32u8).filter(|i| i % 2 == 1).chain((0..32u8).filter(|i| i % 2 == 0)).collect();
        assert_eq!(notes, want);
    }
}


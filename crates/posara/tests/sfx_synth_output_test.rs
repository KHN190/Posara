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


mod sfx_ops;

mod engine_props {
    use super::sfx_ops::{session, Op};
    use posara::sfx::mixer::Cmd;
    use posara::sfx::{AudioMeter, Mixer, DELAY_CAP_MS};
    use posara_core::sfx::bus::{Delay, Reverb};
    use posara_core::sfx::fx::Fx;
    use posara_core::sfx::lfo::Lfo;
    use posara_core::sfx::seq::{by_tick, Event, Sequencer};
    use proptest::prelude::*;
    use std::sync::Arc;

    const SR: u32 = 48_000;
    const BLOCK: usize = 480;

    fn mixer(channels: usize) -> Mixer { Mixer::new(SR, channels, Arc::new(AudioMeter::default()), DELAY_CAP_MS) }

    fn render(m: &mut Mixer, ops: &[(Op, usize)], channels: usize) -> Vec<f32> {
        let mut buf = vec![0f32; BLOCK * channels];
        let mut out = Vec::new();
        for (op, blocks) in ops {
            m.apply(op.cmd());
            for _ in 0..*blocks {
                m.mix(&mut buf);
                out.extend_from_slice(&buf);
            }
        }
        out
    }

    fn freq(note: u8) -> f64 { 440.0 * 2f64.powf((note as f64 - 69.0) / 12.0) }

    fn rising_crossings(x: &[f32]) -> usize { x.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count() }

    proptest! {
        #[test]
        fn any_command_stream_mixes_finite_samples_inside_full_scale(ops in session(), stereo in any::<bool>()) {
            let ch = if stereo { 2 } else { 1 };
            let out = render(&mut mixer(ch), &ops, ch);
            for (i, s) in out.iter().enumerate() {
                prop_assert!(s.is_finite() && s.abs() <= 1.0, "sample {i} = {s}");
            }
        }

        #[test]
        fn mono_output_is_the_mean_of_stereo(ops in session()) {
            let ops: Vec<_> = ops.into_iter().filter(|(o, _)| !o.is_bus_delay()).collect();
            let st = render(&mut mixer(2), &ops, 2);
            let mono = render(&mut mixer(1), &ops, 1);
            for (i, (m, lr)) in mono.iter().zip(st.chunks(2)).enumerate() {
                prop_assert!((m - (lr[0] + lr[1]) * 0.5).abs() <= 1e-6, "frame {i}: {m} vs {lr:?}");
            }
        }

        #[test]
        fn a_mixer_with_no_notes_is_silent(room in 0.0f32..1.0, mix in 0.0f32..1.0, ms in 0u32..2000, fb in 0.0f32..1.0) {
            let mut m = mixer(2);
            m.apply(Cmd::BusDelay(ms, fb, mix));
            m.apply(Cmd::BusReverb(room, 0.5, mix));
            let mut buf = vec![1f32; BLOCK * 2];
            for _ in 0..20 {
                m.mix(&mut buf);
                prop_assert!(buf.iter().all(|&s| s == 0.0));
            }
        }

        #[test]
        fn every_note_is_silent_after_its_gate_and_release(
            notes in proptest::collection::vec((any::<bool>(), 0usize..4, 30u8..100, 1u32..300, 0.0f32..0.3, 0.0f32..0.3, 0u32..20), 1..12),
        ) {
            let mut m = mixer(2);
            let mut end = 0usize;
            let mut t = 0usize;
            for &(synth, ch, note, gate_ms, atk, rel, wait) in &notes {
                if synth {
                    m.apply(Cmd::SynEnv(ch, 0, 0, 100.0, atk, 0.05, 0.6, rel));
                    m.apply(Cmd::SynOn(ch, note, 0.8, gate_ms));
                } else {
                    m.apply(Cmd::Inst(ch, 3, (atk * 1000.0) as u32, 50, 0.6, (rel * 1000.0) as u32));
                    m.apply(Cmd::PlayMidi(ch, note as i64, 0.8, gate_ms));
                }
                let gate = gate_ms as usize * SR as usize / 1000;
                let tail = (rel.max(0.05) as f64 * SR as f64).ceil() as usize;
                end = end.max(t + gate + tail + 2);
                let mut buf = vec![0f32; BLOCK * 2];
                for _ in 0..wait { m.mix(&mut buf); t += BLOCK; }
            }
            let mut buf = vec![0f32; BLOCK * 2];
            while t < end { m.mix(&mut buf); t += BLOCK; }
            for _ in 0..4 {
                m.mix(&mut buf);
                prop_assert!(buf.iter().all(|&s| s == 0.0), "still sounding past {end}");
            }
        }

        #[test]
        fn a_sine_voice_sounds_at_its_equal_tempered_pitch(note in 24u8..100, synth in any::<bool>()) {
            let mut m = mixer(1);
            if synth {
                m.apply(Cmd::SynOsc(0, 0, 1, 0, 0.0, 1.0));
                m.apply(Cmd::SynOn(0, note, 1.0, 0));
            } else {
                m.apply(Cmd::Inst(0, 1, 0, 0, 1.0, 0));
                m.apply(Cmd::PlayMidi(0, note as i64, 1.0, 0));
            }
            let mut x = vec![0f32; SR as usize];
            m.mix(&mut x);
            let got = rising_crossings(&x) as f64;
            prop_assert!((got - freq(note)).abs() <= 2.0, "note {note}: {got} cycles vs {}", freq(note));
        }

        #[test]
        fn fm_stays_bounded_at_any_depth_and_ratio(semi in -24i64..36, depth in 0.0f32..4.0, note in 24u8..110) {
            let mut s = posara::sfx::synth::Synth::new();
            s.osc(0, 0, 1, 0, 0.0, 1.0);
            s.osc(0, 1, 5, semi, 0.0, depth);
            s.note_on(0, note, 1.0, 0);
            for _ in 0..4800 {
                let (l, r) = s.tick(SR as f32);
                prop_assert!(l.is_finite() && r.is_finite() && l.abs() <= 1.0 && r.abs() <= 1.0);
            }
        }

        #[test]
        fn a_delay_line_repeats_an_impulse_every_delay_time(ms in 1u32..500, fb in 0.0f32..0.97, stereo in any::<bool>()) {
            let mut d = Delay::new(SR as usize, stereo);
            d.set(ms, fb, 1.0, SR as f32);
            let len = (ms * SR / 1000) as usize;
            let input = if stereo { 1.0 } else { 0.5 };
            for i in 0..len * 4 + 1 {
                let x = if i == 0 { 1.0 } else { 0.0 };
                let (l, _) = d.process(x, 0.0);
                let want = if i == 0 { 1.0 } else if i % len == 0 { input * fb.powi((i / len) as i32 - 1) } else { 0.0 };
                prop_assert!((l - want).abs() <= 1e-6, "sample {i}: {l} vs {want}");
            }
        }

        #[test]
        fn reverb_tails_stay_finite_and_die_away(room in 0.0f32..1.0, damp in 0.0f32..1.0, burst in proptest::collection::vec(-1.0f32..1.0, 1..2000)) {
            let mut r = Reverb::new(SR as f32);
            r.set(room, damp, 1.0);
            let mut peak = 0.0f32;
            let mut late = 0.0f32;
            for i in 0..SR as usize * 12 {
                let y = r.process(burst.get(i).copied().unwrap_or(0.0));
                prop_assert!(y.is_finite());
                if i < SR as usize { peak = peak.max(y.abs()); } else if i > SR as usize * 11 { late = late.max(y.abs()); }
            }
            prop_assert!(late <= peak * 0.05 + 1e-6, "tail {late} vs onset {peak}");
        }

        #[test]
        fn insert_effects_keep_their_level_bounds(kind in 1u8..6, amt in 0.0f32..1.0, param in 0.0f32..20000.0, xs in proptest::collection::vec(-1.0f32..1.0, 1..400)) {
            let mut fx = Fx::default();
            fx.set(kind, amt, param);
            let peak = xs.iter().fold(0.0f32, |a, x| a.max(x.abs()));
            for &x in &xs {
                let y = fx.process(x, SR as f32);
                let bound = match kind { 1 => 1.0, 2 => 1.0, 3 => peak, 4 => 2.0 * peak, _ => x.abs() };
                prop_assert!(y.is_finite() && y.abs() <= bound + 1e-5, "fx {kind}: {x} → {y} (bound {bound})");
            }
        }

        #[test]
        fn an_lfo_stays_within_depth_and_repeats_each_period(wave in 0u8..4, depth in 0.0f32..1.0, rate in prop::sample::select(vec![1u32, 2, 3, 4, 5, 6, 8, 10, 12, 15, 16, 20])) {
            let mut l = Lfo::default();
            l.set(0, wave, rate as f32, depth);
            let period = (SR / rate) as usize;
            let a: Vec<f32> = (0..period).map(|_| l.tick(SR as f32)).collect();
            let b: Vec<f32> = (0..period).map(|_| l.tick(SR as f32)).collect();
            let drift = period / 500 + 1;
            let near = |i: usize, y: f32| (i + period - drift..=i + period + drift).any(|j| (a[j % period] - y).abs() <= 1e-3 + depth * 1e-3);
            for (i, (x, &y)) in a.iter().zip(&b).enumerate() {
                prop_assert!(x.abs() <= depth + 1e-6);
                prop_assert!(near(i, y), "sample {i}: {y} not near the first period");
            }
        }

        #[test]
        fn sequenced_events_fire_in_order_at_their_tick_time(
            raw in proptest::collection::vec((0u32..64, 0u8..128), 1..40),
            spt in 1.0f32..400.0,
        ) {
            let events: Vec<Event> = raw.iter().map(|&(tick, note)| Event { tick, ch: 0, note, vel: 0.5, dur: 1 }).collect();
            let mut want: Vec<(usize, u8)> = raw.iter().map(|&(t, n)| (t as usize, n)).collect();
            want.sort_by_key(|&(t, _)| t);
            let mut s = Sequencer::empty();
            s.load(by_tick(events), spt);
            let mut got = Vec::new();
            for step in 1..=(64.0 * spt) as usize + 2 {
                s.tick();
                while let Some(e) = s.next_due() { got.push((step, e.note)); }
            }
            prop_assert_eq!(got.iter().map(|g| g.1).collect::<Vec<_>>(), want.iter().map(|w| w.1).collect::<Vec<_>>());
            for (&(step, _), &(tick, _)) in got.iter().zip(&want) {
                let due = (tick as f64 * spt as f64).ceil().max(1.0) as usize;
                prop_assert!(step.abs_diff(due) <= 1, "tick {tick}: fired at {step}, due {due}");
            }
        }
    }
}

#[test]
fn bitcrush_never_overshoots_full_scale() {
    let mut fx = posara_core::sfx::fx::Fx::default();
    fx.set(1, 0.7554858, 0.0);
    assert!(fx.process(0.94240755, 48_000.0) <= 1.0);
}

#[test]
fn an_oscillator_tuned_past_infinity_stays_silent_instead_of_nan() {
    let mut s = Synth::new();
    s.osc(14, 1, 1, 0, 853803.4, 0.99);
    s.filter(14, 1, 0.0, 0.0);
    s.note_on(14, 0, 0.0, 0);
    for _ in 0..480 {
        let (l, r) = s.tick(48_000.0);
        assert!(l.is_finite() && r.is_finite());
    }
}

use posara_core::sfx::synth::{Synth, TGT_AMP_EXP, WAVE_PREVIEW_FM};

fn patched() -> Synth {
    let mut s = Synth::new();
    s.set_voices(16);
    for pid in 0..4 {
        s.osc(pid, 0, 1 + (pid as u8 % 5), 0, 0.0, 0.8);
        s.osc(pid, 1, [2, 5, 3, 6][pid], 7, 3.0, 0.4);
        s.filter(pid, (pid % 3) as u8, 1800.0, 40.0);
        s.env(pid, 0, 0, 1.0, 0.01, 0.2, 0.6, 0.3);
        s.env(pid, 1, 1, 50.0, 0.0, 0.3, 0.2, 0.3);
        s.lfo(pid, 2, 0, 5.0, 0.1);
        s.unison(pid, 1 + pid as u8, 12.0);
        s.fx(pid, pid as u8 % 3, 0.3, 0.5);
        s.modulate(pid, 0.9, 1.0, 0.2 * pid as f32);
    }
    s
}

#[test]
fn block_render_is_bit_identical_to_per_sample_ticks() {
    let (mut a, mut b) = (patched(), patched());
    let mut out = vec![(0.0f32, 0.0f32); 192];
    for block in 0..200usize {
        if block % 7 == 0 {
            for s in [&mut a, &mut b] { s.note_on(block % 4, 40 + (block % 30) as u8, 0.8, 3000 + block as u32 * 13); }
        }
        if block % 11 == 0 { for s in [&mut a, &mut b] { s.note_off((block + 1) % 4, 40 + ((block + 3) % 30) as u8); } }
        let expected: Vec<(f32, f32)> = (0..192).map(|_| a.tick(48000.0)).collect();
        b.render(48000.0, &mut out);
        for (i, (e, g)) in expected.iter().zip(&out).enumerate() {
            assert!(e.0.to_bits() == g.0.to_bits() && e.1.to_bits() == g.1.to_bits(), "block {block} sample {i}: {e:?} != {g:?}");
        }
        assert_eq!(a.take_pid_peaks().map(f32::to_bits), b.take_pid_peaks().map(f32::to_bits));
    }
}

fn builtin(tone: u32) -> Synth {
    let mut s = Synth::new();
    s.set_voices(16);
    for ch in 0..3 {
        s.osc(ch, 0, 1, 0, 0.0, 1.0);
        s.osc(ch, 1, WAVE_PREVIEW_FM, 12, 0.0, (10 + 8 * tone) as f32 / 100.0);
        s.filter(ch, 0, (400 + 60 * tone * tone) as f32, 0.0);
        s.env(ch, 0, TGT_AMP_EXP, 100.0, 0.01, 0.29, 0.0, 0.3);
    }
    s
}

fn render_note(mut s: Synth, note: u8, gate: u32, blocks: usize, block_render: bool) -> Vec<f32> {
    s.note_on(0, note, 0.8, gate);
    let mut out = vec![(0.0f32, 0.0f32); 192];
    let mut mono = Vec::new();
    for _ in 0..blocks {
        if block_render { s.render(48000.0, &mut out); } else { for f in out.iter_mut() { *f = s.tick(48000.0); } }
        mono.extend(out.iter().map(|(l, r)| l + r));
    }
    mono
}

#[test]
fn builtin_kernel_keeps_pitch() {
    for tone in [0, 3, 7] {
        let old = render_note(builtin(tone), 69, 0, 250, false);
        let new = render_note(builtin(tone), 69, 0, 250, true);
        let crossings = |v: &[f32]| v.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count() as i64;
        assert!((crossings(&old) - crossings(&new)).abs() <= 2, "tone {tone}: {} vs {}", crossings(&old), crossings(&new));
    }
}

#[test]
fn builtin_kernel_matches_the_reference_waveform() {
    for tone in [0, 3, 7] {
        let old = render_note(builtin(tone), 57, 30_000, 250, false);
        let new = render_note(builtin(tone), 57, 30_000, 250, true);
        let dot: f64 = old.iter().zip(&new).map(|(a, b)| *a as f64 * *b as f64).sum();
        let (ea, eb): (f64, f64) = (old.iter().map(|a| (*a as f64).powi(2)).sum(), new.iter().map(|b| (*b as f64).powi(2)).sum());
        let corr = dot / (ea * eb).sqrt();
        assert!(corr > 0.99, "tone {tone}: correlation {corr}");
        assert!((eb / ea).sqrt() > 0.97 && (eb / ea).sqrt() < 1.03, "tone {tone}: level ratio {}", (eb / ea).sqrt());
    }
}

#[test]
fn builtin_kernel_starts_and_ends_without_steps() {
    let new = render_note(builtin(3), 45, 10_000, 120, true);
    let peak = new.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    let worst = new.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0f32, f32::max);
    assert!(worst < 0.25 * peak, "largest sample step {worst} vs peak {peak}");
    assert!(new[..4].iter().all(|v| v.abs() < 0.02 * peak), "onset jumps: {:?}", &new[..4]);
}

#[test]
fn builtin_patch_takes_the_specialised_kernel() {
    let mut s = builtin(3);
    s.note_on(0, 60, 0.8, 4000);
    let mut out = vec![(0.0f32, 0.0f32); 192];
    let before = posara_core::sfx::synth::KERNEL_PROBE.load(core::sync::atomic::Ordering::Relaxed);
    s.render(48000.0, &mut out);
    assert!(posara_core::sfx::synth::KERNEL_PROBE.load(core::sync::atomic::Ordering::Relaxed) > before);
}

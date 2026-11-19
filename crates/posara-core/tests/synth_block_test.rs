use posara_core::sfx::synth::Synth;

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

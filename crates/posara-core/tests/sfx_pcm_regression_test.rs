#![cfg(all(feature = "synth", feature = "std"))]

use posara_core::sfx::synth::Synth;

fn render_scenario() -> Vec<f32> {
    let mut synth = Synth::new();
    let sr = 44100.0;
    let mut output = Vec::with_capacity(4096 * 2);

    // Patch 0: Unison + Bitcrush + Pan
    synth.osc(0, 0, 1, 0, 0.0, 0.8);
    synth.osc(0, 1, 1, 12, 0.0, 0.2);
    synth.unison(0, 2, 10.0);
    synth.filter(0, 1, 2000.0, 20.0);
    synth.pan(0, -0.5);
    synth.fx(0, 1, 0.3, 0.0); // Bitcrush

    // Patch 1: Filter + Pan right
    synth.osc(1, 0, 2, 0, 0.0, 1.0);
    synth.filter(1, 1, 800.0, 50.0);
    synth.pan(1, 0.7);

    // 1. Idle render (no notes)
    for _ in 0..512 {
        let (l, r) = synth.tick(sr);
        output.push(l);
        output.push(r);
    }

    // 2. Note on Patch 0
    synth.note_on(0, 60, 0.9, 0);
    for _ in 0..1024 {
        let (l, r) = synth.tick(sr);
        output.push(l);
        output.push(r);
    }

    // 3. Note on Patch 1 and Note off Patch 0
    synth.note_on(1, 67, 0.8, 0);
    synth.note_off(0, 60);
    for _ in 0..1024 {
        let (l, r) = synth.tick(sr);
        output.push(l);
        output.push(r);
    }

    // 4. Voice reuse and parameter update
    synth.pan(1, -0.2);
    synth.note_on(1, 72, 0.7, 500);
    for _ in 0..1536 {
        let (l, r) = synth.tick(sr);
        output.push(l);
        output.push(r);
    }

    output
}

#[test]
fn sfx_pcm_regression_output_is_deterministic() {
    let run1 = render_scenario();
    let run2 = render_scenario();
    assert_eq!(run1.len(), run2.len());
    for (i, (s1, s2)) in run1.iter().zip(run2.iter()).enumerate() {
        assert_eq!(s1.to_bits(), s2.to_bits(), "Mismatch at sample {}", i);
    }

    // Verify non-silent output when playing notes
    let non_zero = run1.iter().filter(|&&s| s != 0.0).count();
    assert!(non_zero > 1000, "Expected active audio output, got {} non-zero samples", non_zero);

    // Verify first 512 samples (idle) are strictly zero
    for (i, &s) in run1[..1024].iter().enumerate() {
        assert_eq!(s, 0.0, "Expected silence during idle, got {} at sample {}", s, i);
    }
}

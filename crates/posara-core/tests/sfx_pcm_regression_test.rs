#![cfg(all(feature = "synth", feature = "std"))]

use posara_core::sfx::synth::Synth;

fn render_scenario() -> Vec<f32> {
    let mut synth = Synth::new();
    let mut sr = 44100.0;
    let mut output = Vec::with_capacity(8192 * 2);

    // Patch 0: FM synth + LFO cutoff modulation + Bitcrush + Pan
    synth.osc(0, 0, 1, 0, 0.0, 0.8);
    synth.osc(0, 1, 5, 12, 0.0, 1.5); // WAVE_FM = 5
    synth.lfo(0, 1, 1, 4.0, 50.0); // target=CUTOFF, wave=SINE, rate=4Hz, depth=50
    synth.filter(0, 1, 2000.0, 20.0);
    synth.pan(0, -0.5);
    synth.fx(0, 1, 0.3, 0.0); // Bitcrush

    // Patch 1: Filter + Pan right + LFO pitch modulation
    synth.osc(1, 0, 2, 0, 0.0, 1.0);
    synth.lfo(1, 2, 1, 6.0, 2.0); // target=PITCH
    synth.filter(1, 1, 800.0, 50.0);
    synth.pan(1, 0.7);

    // Patch 2: Long envelope release
    synth.osc(2, 0, 3, 0, 0.0, 0.7);
    synth.env(2, 0, 0, 100.0, 10.0, 50.0, 0.5, 300.0); // long release 300ms

    // 1. Idle render (no notes)
    for _ in 0..512 {
        let (l, r) = synth.tick(sr);
        output.push(l);
        output.push(r);
    }

    // 2. Note on Patch 0 (FM + Bitcrush)
    synth.note_on(0, 60, 0.9, 0);
    for _ in 0..1024 {
        let (l, r) = synth.tick(sr);
        output.push(l);
        output.push(r);
    }

    // 3. Sample rate change to 22050 + Note on Patch 1
    sr = 22050.0;
    synth.note_on(1, 67, 0.8, 0);
    for _ in 0..1024 {
        let (l, r) = synth.tick(sr);
        output.push(l);
        output.push(r);
    }

    // 4. Note on Patch 2 + Note off Patch 0 & 1 (Release phase test)
    synth.note_on(2, 72, 0.8, 0);
    synth.note_off(0, 60);
    synth.note_off(1, 67);
    for _ in 0..1024 {
        let (l, r) = synth.tick(sr);
        output.push(l);
        output.push(r);
    }

    // 5. Sample rate change to 48000 + Voice reuse (pool = 4, play 8 notes)
    sr = 48000.0;
    synth.set_voices(4);
    for n in 0..8 {
        synth.note_on(2, 60 + n * 2, 0.7, 200);
    }
    for _ in 0..1536 {
        let (l, r) = synth.tick(sr);
        output.push(l);
        output.push(r);
    }

    output
}

#[test]
fn pcm_matches_unoptimized_7d9c4e8() {
    let run1 = render_scenario();
    let h = pcm_hash(&run1);
    // Oracle: 7d9c4e8 with symmetric center pan; identical math dependencies.
    const EXPECTED_BASELINE_HASH: u64 = 0x9d1dd59e7e18f1e2;
    assert_eq!(h, EXPECTED_BASELINE_HASH, "PCM differs from the unoptimized oracle");

    // Verify non-silent output when playing notes
    let non_zero = run1.iter().filter(|&&s| s != 0.0).count();
    assert!(non_zero > 1000, "Expected active audio output, got {} non-zero samples", non_zero);

    // Verify first 512 samples (idle) are strictly zero
    for (i, &s) in run1[..1024].iter().enumerate() {
        assert_eq!(s, 0.0, "Expected silence during idle, got {} at sample {}", s, i);
    }
}

fn pcm_hash(samples: &[f32]) -> u64 {
    samples.iter().flat_map(|sample| sample.to_bits().to_le_bytes()).fold(
        0xcbf29ce484222325u64,
        |hash, byte| (hash ^ byte as u64).wrapping_mul(0x100000001b3),
    )
}

#[test]
fn fx_cache_matches_unoptimized_parameter_and_rate_changes() {
    // Oracle: 7d9c4e8, zero-depth bypass applied to the reference trace.
    assert_eq!(pcm_hash(&fx_key_trace()), 0x001380d277dd5765);
}

fn fx_key_trace() -> Vec<f32> {
    use posara_core::sfx::fx::Fx;
    let mut fx = Fx::default();
    let mut output = Vec::new();
    let mut seed = 0x12345678u32;
    for (kind, amount, parameter, rate) in [
        (1, 0.3, 0.0, 48000.0), (1, 0.3, 0.0, 48000.0),
        (1, 0.7, 0.0, 48000.0), (1, 0.0, 0.0, 44100.0),
        (3, 0.5, 800.0, 48000.0), (3, 0.5, 800.0, 22050.0),
        (4, 0.5, 800.0, 22050.0), (4, 0.5, 1800.0, 96000.0),
        (3, 0.5, 800.0, 48000.0), (3, 0.5, -0.0, 48000.0),
        (3, 0.5, 0.0, 48000.0), (1, 0.3, 0.0, 48000.0),
    ] {
        fx.kind = kind;
        fx.amt = amount;
        fx.param = parameter;
        for _ in 0..64 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let x = (seed >> 9) as f32 / 8388608.0 * 2.0 - 1.0;
            output.push(fx.process(x, rate));
        }
    }
    output
}

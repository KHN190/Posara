#![cfg(feature = "audio-profile")]
use posara_core::sfx::profiling::{self, Stage};

#[test]
fn every_sample_has_one_stage_and_out_of_range_samples_stay_in_the_denominator() {
    let before = profiling::snapshot();
    profiling::mark(Stage::Sequence);
    profiling::sample(true);
    profiling::sample(true);
    profiling::mark(Stage::SynthPan);
    profiling::sample(false);
    let after = profiling::snapshot();
    assert_eq!(after.total - before.total, 3);
    assert_eq!(after.out_of_range - before.out_of_range, 1);
    assert_eq!(after.counts[Stage::Sequence as usize] - before.counts[Stage::Sequence as usize], 2);
    assert_eq!(after.counts[Stage::SynthPan as usize] - before.counts[Stage::SynthPan as usize], 1);
    assert_eq!(after.counts.iter().sum::<u32>(), after.total);
}

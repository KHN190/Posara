#![cfg(feature = "sfx")]

#[test]
fn every_note_preserves_backend_frequency_bits() {
    let mut previous = 0.0;
    for note in 0..=u8::MAX {
        let expected = 440.0 * posara_core::math::powf(2.0, (note as f32 - 69.0) / 12.0);
        let actual = posara_core::sfx::seq::note_to_freq(note);
        assert_eq!(actual.to_bits(), expected.to_bits(), "note={note}");
        assert!(actual.is_finite() && actual > previous, "note={note}");
        previous = actual;
    }
    assert_eq!(posara_core::sfx::seq::note_to_freq(69), 440.0);
}

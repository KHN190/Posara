use posara_core::audio::to_mono_i16;

#[test]
fn averages_left_right() {
    let mut out = [0i16; 3];
    to_mono_i16(&[1.0, 1.0, 1.0, -1.0, -0.5, -0.5], &mut out);
    assert_eq!(out, [32767, 0, -16383]);
}

#[test]
fn clamps_out_of_range() {
    let mut out = [0i16; 2];
    to_mono_i16(&[2.0, 2.0, -3.0, -3.0], &mut out);
    assert_eq!(out, [32767, -32767]);
}

#[test]
fn writes_min_of_both_lengths() {
    let mut out = [7i16; 3];
    to_mono_i16(&[0.0, 0.0], &mut out);
    assert_eq!(out, [0, 7, 7]);
}

#[test]
fn mono_sample_scales_and_clamps() {
    use posara_core::audio::to_i16;
    assert_eq!([to_i16(0.5), to_i16(-1.5), to_i16(1.0)], [16383, -32767, 32767]);
}

#[test]
fn widened_right_channel_lags_by_the_width() {
    use posara_core::audio::Width;
    let mut w = Width::new(2);
    w.set(true);
    let out: Vec<_> = [1.0, 2.0, 3.0, 4.0].iter().map(|&x| w.next(x)).collect();
    assert_eq!(out, [(1.0, 0.0), (2.0, 0.0), (3.0, 1.0), (4.0, 2.0)]);
}

#[test]
fn narrow_output_is_the_same_on_both_sides() {
    use posara_core::audio::Width;
    let mut w = Width::new(2);
    assert_eq!([w.next(1.0), w.next(-0.5)], [(1.0, 1.0), (-0.5, -0.5)]);
}

#[test]
fn widening_mid_stream_lags_from_the_signal_already_heard() {
    use posara_core::audio::Width;
    let mut w = Width::new(1);
    w.next(1.0);
    w.set(true);
    assert_eq!(w.next(2.0), (2.0, 1.0));
}

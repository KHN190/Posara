use posara_core::drivers::{Imu, MidiPort, NullImu, NullMidi};

#[test]
fn null_midi_is_silent() {
    let mut m = NullMidi;
    assert_eq!(m.recv(), None);
    assert!(m.send(0x90 + 60 * 256 + 100 * 65536).is_ok());
    assert_eq!(m.recv(), None);
}

#[test]
fn null_imu_reads_zero() {
    assert_eq!(NullImu.accel(), [0, 0, 0]);
}

use posara_usb_midi::{packet, word};

#[test]
fn note_on_packet_becomes_word_tagged_with_source() {
    assert_eq!(word([0x09, 0x90, 60, 100], 1), Some(0x90 | 60 << 8 | 100 << 16 | 1 << 24));
}

#[test]
fn reserved_code_index_is_ignored() {
    assert_eq!(word([0x00, 0x90, 60, 100], 1), None);
}

#[test]
fn word_becomes_packet_on_cable_zero() {
    assert_eq!(packet(0x90 | 60 << 8 | 100 << 16), [0x09, 0x90, 60, 100]);
    assert_eq!(packet(0x80 | 60 << 8), [0x08, 0x80, 60, 0]);
}

#[test]
fn two_byte_messages_drop_the_third_byte() {
    assert_eq!(packet(0xC3 | 7 << 8 | 99 << 16), [0x0C, 0xC3, 7, 0]);
    assert_eq!(packet(0xD0 | 64 << 8 | 99 << 16), [0x0D, 0xD0, 64, 0]);
}

#[test]
fn realtime_is_a_single_byte_packet() {
    for s in [0xF8u32, 0xFA, 0xFB, 0xFC] {
        assert_eq!(packet(s | 5 << 8 | 6 << 16 | 2 << 24), [0x0F, s as u8, 0, 0]);
    }
}

#[test]
fn round_trip_keeps_channel_messages() {
    for w in [0x90 | 60 << 8 | 100 << 16, 0xB1 | 7 << 8 | 127 << 16, 0xE2 | 0x7F << 16] {
        assert_eq!(word(packet(w), 0), Some(w));
    }
}

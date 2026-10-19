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

use posara_core::midi::message;
use proptest::prelude::*;

// MIDI 1.0 status table: data bytes after the status, None = not a sendable message on its own.
fn data_bytes(status: u8) -> Option<usize> {
    match status {
        0x80..=0x8F | 0x90..=0x9F | 0xA0..=0xAF | 0xB0..=0xBF | 0xE0..=0xEF => Some(2),
        0xC0..=0xCF | 0xD0..=0xDF => Some(1),
        0xF1 | 0xF3 => Some(1),
        0xF2 => Some(2),
        0xF6 | 0xF8 | 0xFA | 0xFB | 0xFC | 0xFE | 0xFF => Some(0),
        0xF9 | 0xFD => Some(0),
        _ => None,
    }
}

// USB MIDI 1.0 table 4-1 code index numbers.
fn cin(status: u8) -> u8 {
    match status {
        0x80..=0xEF => status >> 4,
        0xF1 | 0xF3 => 0x2,
        0xF2 => 0x3,
        0xF6 => 0x5,
        0xF8..=0xFF => 0xF,
        _ => 0x0,
    }
}

proptest! {
    #[test]
    fn every_status_sends_exactly_its_spec_length(w in any::<u32>()) {
        let (bytes, len) = message(w);
        let status = w as u8;
        prop_assert_eq!(len, data_bytes(status).map_or(0, |n| n + 1));
        if len > 0 { prop_assert_eq!(bytes[0], status); }
    }

    #[test]
    fn sent_data_bytes_never_look_like_a_status(w in any::<u32>()) {
        let (bytes, len) = message(w);
        for b in &bytes[1..len.max(1)] { prop_assert!(*b < 0x80, "{w:#x} → {bytes:?}"); }
    }

    #[test]
    fn usb_packets_carry_the_spec_code_index(w in any::<u32>()) {
        let p = packet(w);
        let status = w as u8;
        match data_bytes(status) {
            Some(n) => {
                prop_assert_eq!(p[0], cin(status));
                prop_assert_eq!(p[1], status);
                prop_assert_eq!(&p[2..2 + n], &[(w >> 8) as u8 & 0x7F, (w >> 16) as u8 & 0x7F][..n]);
                prop_assert!(p[2 + n..].iter().all(|&b| b == 0));
            }
            None => prop_assert_eq!(p, [0, 0, 0, 0]),
        }
    }

    #[test]
    fn a_packet_read_back_is_the_word_tagged_with_its_source(st in 0x80u8..=0xFF, d1 in 0u8..0x80, d2 in 0u8..0x80, src in any::<u8>()) {
        let n = data_bytes(st);
        prop_assume!(n.is_some());
        let n = n.unwrap();
        let w = st as u32 | (d1 as u32) << 8 | (d2 as u32) << 16;
        let kept = [d1, d2];
        let want = st as u32 | (kept[..n].iter().enumerate().map(|(i, &b)| (b as u32) << (8 * (i + 1))).sum::<u32>()) | (src as u32) << 24;
        prop_assert_eq!(word(packet(w), src), Some(want));
    }
}

#[test]
fn a_data_byte_as_status_sends_nothing() {
    assert_eq!(message(0x45).1, 0);
    assert_eq!(packet(0x45), [0; 4]);
}

#[test]
fn data_bytes_over_127_are_masked_to_seven_bits() {
    let (bytes, len) = message(0x90 | 0xBC << 8 | 0xE4 << 16);
    assert_eq!(&bytes[..len], &[0x90, 0x3C, 0x64]);
}

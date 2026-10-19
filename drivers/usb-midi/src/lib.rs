#![no_std]

// Word = status | d1 << 8 | d2 << 16 | src << 24 (posara MIDI device format).
pub fn word(p: [u8; 4], src: u8) -> Option<u32> {
    if p[0] & 0x0F == 0 { return None; }
    Some(p[1] as u32 | (p[2] as u32) << 8 | (p[3] as u32) << 16 | (src as u32) << 24)
}

pub fn packet(w: u32) -> [u8; 4] {
    let [status, d1, d2, _] = w.to_le_bytes();
    let (cin, len) = match status {
        0x80..=0xBF | 0xE0..=0xEF => (status >> 4, 3),
        0xC0..=0xDF => (status >> 4, 2),
        0xF2 => (0x3, 3),
        0xF1 | 0xF3 => (0x2, 2),
        _ => (0xF, 1),
    };
    [cin, status, if len > 1 { d1 } else { 0 }, if len > 2 { d2 } else { 0 }]
}

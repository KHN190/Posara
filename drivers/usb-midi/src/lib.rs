#![no_std]

// Word = status | d1 << 8 | d2 << 16 | src << 24 (posara MIDI device format).
pub fn word(p: [u8; 4], src: u8) -> Option<u32> {
    if p[0] & 0x0F == 0 { return None; }
    Some(p[1] as u32 | (p[2] as u32) << 8 | (p[3] as u32) << 16 | (src as u32) << 24)
}

pub fn packet(w: u32) -> [u8; 4] {
    let ([status, d1, d2], len) = posara_core::midi::message(w);
    let cin = match (status, len) {
        (0x80..=0xEF, _) => status >> 4,
        (_, 1) => 0xF,
        _ => len as u8,
    };
    [cin, status, if len > 1 { d1 } else { 0 }, if len > 2 { d2 } else { 0 }]
}

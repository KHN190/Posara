// 1bpp packing shared by `sprite --bpp 1` and `font`: W*H bits, row-major,
// MSB-first, ceil(bits/8) bytes — the exact layout blit/blitg consume.
pub fn pack_1bpp(ink: &[bool]) -> Vec<u8> {
    let mut bytes = vec![0u8; (ink.len() + 7) / 8];
    for (bit, &on) in ink.iter().enumerate() {
        if on {
            bytes[bit / 8] |= 1 << (7 - (bit & 7));
        }
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::pack_1bpp;

    #[test]
    fn msb_first_roundtrip() {
        // bit 0 = MSB of byte 0; bit 7 = LSB; bit 8 = MSB of byte 1.
        let ink = [true, false, false, false, false, false, false, true, true];
        let b = pack_1bpp(&ink);
        assert_eq!(b.len(), 2);
        assert_eq!(b[0], 0b1000_0001);
        assert_eq!(b[1], 0b1000_0000);
    }

    #[test]
    fn empty_and_partial_byte() {
        assert_eq!(pack_1bpp(&[]).len(), 0);
        assert_eq!(pack_1bpp(&[true]), vec![0b1000_0000]);
    }
}

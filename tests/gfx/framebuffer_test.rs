#![cfg(feature = "gfx")]

use posara::Framebuffer;

fn fb(w: usize, h: usize) -> Framebuffer {
    let mut f = Framebuffer::new();
    f.set_headless();
    f.configure(w, h, 1).unwrap();
    f
}

#[test]
fn configure_rejects_bad_format_and_size() {
    let mut f = Framebuffer::new();
    f.set_headless();
    assert!(f.configure(8, 8, 0).is_err());
    assert!(f.configure(0, 8, 1).is_err());
}

#[test]
fn pset_ignores_out_of_bounds() {
    let mut f = fb(4, 4);
    f.pset(-1, 0, 9);
    f.pset(0, -1, 9);
    f.pset(4, 0, 9);
    f.pset(0, 4, 9);
    assert!(f.buf.iter().all(|&p| p == 0));
    f.pset(2, 1, 9);
    assert_eq!(f.buf[1 * 4 + 2], 9);
}

#[test]
fn pset_op_modes() {
    let mut f = fb(1, 1);
    f.pset(0, 0, 0b1100);
    f.pset_op(0, 0, 0b1010, 1); // XOR
    assert_eq!(f.buf[0], 0b0110);
    f.cls(0b1100);
    f.pset_op(0, 0, 0b1010, 2); // AND
    assert_eq!(f.buf[0], 0b1000);
    f.cls(0b1100);
    f.pset_op(0, 0, 0b1010, 3); // OR
    assert_eq!(f.buf[0], 0b1110);
    f.cls(0b1100);
    f.pset_op(0, 0, 0b1010, 0); // REPLACE
    assert_eq!(f.buf[0], 0b1010);
}

#[test]
fn rect_mix_alpha_endpoints() {
    let mut f = fb(2, 1);
    f.cls(0xF800); // red
    f.rect_mix(0, 0, 2, 1, 0x001F, 0); // a=0: no change
    assert_eq!(f.buf[0], 0xF800);
    f.rect_mix(0, 0, 2, 1, 0x001F, 256); // a=256: full source
    assert_eq!(f.buf[0], 0x001F);
}

#[test]
fn rect_mix_clamps_and_clips() {
    let mut f = fb(2, 2);
    f.cls(0);
    f.rect_mix(-1, -1, 5, 5, 0xFFFF, 999); // over-large alpha + out-of-bounds rect
    assert!(f.buf.iter().all(|&p| p == 0xFFFF));
}

#[test]
fn blit_4bpp_places_nibbles_and_skips_index0() {
    let mut f = fb(4, 4);
    f.palette[1] = 0xABCD;
    f.palette[2] = 0x1234;
    let data: [u8; 2] = [0x10, 0x21];
    f.blit_4bpp(|i| data.get(i).copied().unwrap_or(0), 0, 1, 1, 2, 2, 100, 256);
    assert_eq!(f.buf[1 * 4 + 1], 0xABCD); // idx 1
    assert_eq!(f.buf[1 * 4 + 2], 0);      // idx 0 transparent, untouched
    assert_eq!(f.buf[2 * 4 + 1], 0x1234); // idx 2
    assert_eq!(f.buf[2 * 4 + 2], 0xABCD); // idx 1
}

#[test]
fn blit_4bpp_clips_out_of_bounds() {
    let mut f = fb(2, 2);
    f.palette[1] = 0x7777;
    let data = [0x11u8; 8]; // 4x4 sprite, all index 1
    f.blit_4bpp(|i| data.get(i).copied().unwrap_or(0), 0, -1, -1, 4, 4, 100, 256); // spills all sides
    assert_eq!(f.buf[0], 0x7777); // only the in-bounds intersection written
    assert!(f.buf.iter().all(|&p| p == 0x7777));
}

#[test]
fn cls_fills_whole_buffer() {
    let mut f = fb(3, 3);
    f.cls(0x1234);
    assert!(f.buf.iter().all(|&p| p == 0x1234));
}

#[test]
fn dither_solid_bright_goes_light() {
    let mut f = fb(4, 4);
    f.cls(0xFFFF);
    f.dither(0x0000, 0xFFFF);
    assert!(f.buf.iter().all(|&p| p == 0xFFFF));
}

#[test]
fn dither_solid_dark_goes_dark() {
    let mut f = fb(4, 4);
    f.cls(0x0000);
    f.dither(0x0000, 0xFFFF);
    assert!(f.buf.iter().all(|&p| p == 0x0000));
}

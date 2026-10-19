#![cfg(feature = "gfx")]

use posara_core::gfx::Framebuffer;

#[test]
fn commit_skips_rgba_snapshot_unless_asked() {
    let mut fb = Framebuffer::new();
    fb.configure(128, 64, 1).unwrap();
    fb.commit().unwrap();
    assert!(fb.out.is_empty());
}

#[test]
fn rgba_snapshot_follows_each_commit() {
    let mut fb = Framebuffer::new();
    fb.rgba = true;
    fb.configure(2, 1, 1).unwrap();
    fb.pset(0, 0, 0xFFFF);
    fb.commit().unwrap();
    assert_eq!(fb.out, vec![0xFFFF_FFFF, 0xFF00_0000]);
}

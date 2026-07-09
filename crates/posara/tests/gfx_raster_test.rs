#![cfg(feature = "gfx")]

use posara::Framebuffer;

fn fb(w: usize, h: usize) -> Framebuffer {
    let mut f = Framebuffer::new();
    f.set_headless();
    f.configure(w, h, 1).unwrap();
    f
}

fn lit(f: &Framebuffer) -> usize {
    f.buf.iter().filter(|&&p| p != 0).count()
}

#[test]
fn line_horizontal_sets_each_pixel() {
    let mut f = fb(8, 8);
    f.line(1, 3, 6, 3, 9);
    assert_eq!(lit(&f), 6);
    for x in 1..=6 {
        assert_eq!(f.buf[3 * 8 + x], 9);
    }
}

#[test]
fn line_fully_outside_draws_nothing() {
    let mut f = fb(8, 8);
    f.line(-50, -50, -10, -10, 9);
    assert_eq!(lit(&f), 0);
}

#[test]
fn line_partly_outside_is_clipped_in_bounds() {
    let mut f = fb(8, 8);
    f.line(-5, 4, 12, 4, 9); // extends past both edges
    assert_eq!(lit(&f), 8);  // exactly one full row, nothing out of bounds
}

#[test]
fn line_extreme_coords_rejected_without_hang() {
    let mut f = fb(8, 8);
    f.line(i64::MIN / 2, 0, i64::MAX / 2, 7, 9);
    assert_eq!(lit(&f), 0);
}

#[test]
fn circ_fill_covers_center() {
    let mut f = fb(16, 16);
    f.circ(8, 8, 4, 9, true);
    assert_eq!(f.buf[8 * 16 + 8], 9);
    assert!(lit(&f) > 12);
}

#[test]
fn circ_outline_is_symmetric_and_hollow() {
    let mut f = fb(16, 16);
    f.circ(8, 8, 5, 9, false);
    assert_eq!(f.buf[8 * 16 + 8], 0); // center untouched
    assert_eq!(f.buf[8 * 16 + 13], 9); // +r on x axis
    assert_eq!(f.buf[8 * 16 + 3], 9);  // -r on x axis
}

#[test]
fn rect_outline_draws_border_only() {
    let mut f = fb(8, 8);
    f.rect_outline(1, 1, 4, 4, 9);
    assert_eq!(f.buf[1 * 8 + 1], 9); // corner
    assert_eq!(f.buf[2 * 8 + 2], 0); // interior empty
    assert_eq!(lit(&f), 12);         // 4*4 outline = 4w+4h-4
}

#[test]
fn tri_fill_includes_interior_point() {
    let mut f = fb(16, 16);
    f.tri(2, 2, 12, 2, 2, 12, 9, false);
    let outline = lit(&f);
    let mut g = fb(16, 16);
    g.tri(2, 2, 12, 2, 2, 12, 9, true);
    assert!(lit(&g) > outline);
    assert_eq!(g.buf[4 * 16 + 4], 9); // inside the triangle
}

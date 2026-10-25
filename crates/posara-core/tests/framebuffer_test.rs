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

mod rotation {
    use std::collections::{HashMap, HashSet};

    use posara_core::gfx::{blitr_map, circle_span, sin_q16};
    use proptest::prelude::*;

    fn map(w: i64, h: i64, deg: i64) -> HashMap<(i64, i64), (i64, i64)> {
        let mut m = HashMap::new();
        blitr_map(w, h, deg, |dx, dy, sx, sy| { assert!(m.insert((dx, dy), (sx, sy)).is_none()); });
        m
    }

    fn every_pixel_once(m: &HashMap<(i64, i64), (i64, i64)>, w: i64, h: i64) -> bool {
        let seen: HashSet<_> = m.values().copied().collect();
        seen.len() == m.len() && m.len() == (w * h) as usize
    }

    #[test]
    fn the_sine_table_rounds_each_degree_to_q16() {
        for d in 0..360 {
            let want = ((d as f64).to_radians().sin() * 65536.0).round() as i32;
            assert_eq!(sin_q16(d), want, "{d}°");
        }
    }

    proptest! {
        #[test]
        fn no_turn_draws_every_pixel_in_place(w in 1i64..96, h in 1i64..96) {
            let m = map(w, h, 0);
            prop_assert!(every_pixel_once(&m, w, h));
            for (&(dx, dy), &(sx, sy)) in &m { prop_assert_eq!((dx, dy), (sx - w / 2, sy - h / 2)); }
        }

        #[test]
        fn a_half_turn_mirrors_every_pixel(w in 1i64..96, h in 1i64..96) {
            let m = map(w, h, 180);
            prop_assert!(every_pixel_once(&m, w, h));
            for (&(dx, dy), &(sx, sy)) in &m { prop_assert_eq!((sx, sy), (w - 1 - (dx + w / 2), h - 1 - (dy + h / 2))); }
        }

        #[test]
        fn quarter_turns_of_matching_parity_draw_every_pixel_once(hw in 0i64..24, hh in 0i64..24, odd in 1i64..=2, q in 0i64..4) {
            let (w, h) = (hw * 2 + odd, hh * 2 + odd);
            prop_assert!(every_pixel_once(&map(w, h, q * 90), w, h));
        }

        #[test]
        fn a_whole_turn_changes_nothing(w in 1i64..64, h in 1i64..64, deg in -720i64..720) {
            let m = map(w, h, deg);
            prop_assert_eq!(&m, &map(w, h, deg + 360));
            prop_assert_eq!(&m, &map(w, h, deg - 360));
        }

        #[test]
        fn filled_circle_spans_are_the_integer_square_root(r in 0i64..1_000_000, t in 0.0f64..=1.0) {
            let dy = (r as f64 * t) as i64;
            let n = r * r + r - dy * dy;
            let s = circle_span(r, dy);
            prop_assert!(s * s <= n && (s + 1) * (s + 1) > n, "r {r} dy {dy}: {s}");
        }
    }
}

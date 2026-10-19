#![cfg(feature = "gfx")]

use posara::Framebuffer;

fn fb(w: usize, h: usize) -> Framebuffer {
    let mut f = Framebuffer::new();
    f.set_headless();
    f.configure(w, h, 1).unwrap();
    f
}

mod model {
    use proptest::prelude::*;
    use super::fb;

    pub type Px = std::collections::BTreeSet<(i64, i64)>;

    pub fn lit_at(f: &posara::Framebuffer) -> Px {
        (0..f.buf.len()).filter(|&i| f.buf[i] != 0).map(|i| ((i % f.w) as i64, (i / f.w) as i64)).collect()
    }

    pub fn on_screen(p: Px, w: usize, h: usize) -> Px {
        p.into_iter().filter(|&(x, y)| x >= 0 && y >= 0 && x < w as i64 && y < h as i64).collect()
    }

    pub fn bresenham(x0: i64, y0: i64, x1: i64, y1: i64) -> Px {
        let mut out = Px::new();
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let (mut x, mut y, mut err) = (x0, y0, dx + dy);
        loop {
            out.insert((x, y));
            if x == x1 && y == y1 { return out; }
            let e2 = 2 * err;
            if e2 >= dy { err += dy; x += sx; }
            if e2 <= dx { err += dx; y += sy; }
        }
    }

    fn coord() -> impl Strategy<Value = i64> { -60i64..100 }
    fn size() -> impl Strategy<Value = (usize, usize)> { (1usize..48, 1usize..40) }

    fn check_clipped_line(w: usize, h: usize, x0: i64, y0: i64, x1: i64, y1: i64) -> Result<(), TestCaseError> {
        let mut f = fb(w, h);
        f.line(x0, y0, x1, y1, 1);
        prop_assert_eq!(lit_at(&f), on_screen(bresenham(x0, y0, x1, y1), w, h));
        Ok(())
    }

    fn check_filled_tri(w: usize, h: usize, [x0, y0, x1, y1, x2, y2]: [i64; 6]) -> Result<(), TestCaseError> {
        let mut f = fb(w, h);
        f.tri(x0, y0, x1, y1, x2, y2, 1, true);
        let side = |ax: i64, ay: i64, bx: i64, by: i64, x: i64, y: i64| ((bx - ax) as i128 * (y - ay) as i128 - (by - ay) as i128 * (x - ax) as i128).signum();
        let mut want = Px::new();
        for y in 0..h as i64 {
            for x in 0..w as i64 {
                let s = [side(x1, y1, x2, y2, x, y), side(x2, y2, x0, y0, x, y), side(x0, y0, x1, y1, x, y)];
                let in_box = x >= x0.min(x1).min(x2) && x <= x0.max(x1).max(x2) && y >= y0.min(y1).min(y2) && y <= y0.max(y1).max(y2);
                if in_box && (s.iter().all(|&k| k >= 0) || s.iter().all(|&k| k <= 0)) { want.insert((x, y)); }
            }
        }
        prop_assert_eq!(lit_at(&f), want);
        let mut g = fb(w, h);
        g.tri(x2, y2, x0, y0, x1, y1, 1, true);
        prop_assert_eq!(lit_at(&g), lit_at(&f));
        Ok(())
    }

    proptest! {
        #[test]
        fn a_clipped_line_lights_the_on_screen_part_of_the_whole_line((w, h) in size(), x0 in coord(), y0 in coord(), x1 in coord(), y1 in coord()) {
            check_clipped_line(w, h, x0, y0, x1, y1)?;
            check_clipped_line(7, 32, 3, 21, 0, 32)?;
            check_clipped_line(1, 2, -17, 60, 16, -55)?;
        }

        #[test]
        fn a_line_is_the_same_drawn_from_either_end_when_on_screen(((w, h), (x0, y0, x1, y1)) in size().prop_flat_map(|(w, h)| (Just((w, h)), (0..w as i64, 0..h as i64, 0..w as i64, 0..h as i64)))) {
            let (mut a, mut b) = (fb(w, h), fb(w, h));
            a.line(x0, y0, x1, y1, 1);
            b.line(x1, y1, x0, y0, 1);
            prop_assert!(lit_at(&a).contains(&(x0, y0)) && lit_at(&a).contains(&(x1, y1)));
            prop_assert_eq!(lit_at(&a).len(), lit_at(&b).len());
        }

        #[test]
        fn a_thick_line_is_a_square_brush_along_the_whole_line((w, h) in size(), x0 in coord(), y0 in coord(), x1 in coord(), y1 in coord(), t in 1i64..6) {
            let mut f = fb(w, h);
            f.line_thick(x0, y0, x1, y1, t, 1);
            let brush: Px = bresenham(x0, y0, x1, y1).into_iter()
                .flat_map(|(x, y)| (0..t).flat_map(move |oy| (0..t).map(move |ox| (x - t / 2 + ox, y - t / 2 + oy)))).collect();
            prop_assert_eq!(lit_at(&f), on_screen(brush, w, h));
        }

        #[test]
        fn rect_outline_is_its_four_edges((w, h) in size(), x in coord(), y in coord(), rw in -3i64..60, rh in -3i64..60) {
            let mut f = fb(w, h);
            f.rect_outline(x, y, rw, rh, 1);
            let mut want = Px::new();
            if rw > 0 && rh > 0 {
                for i in 0..rw { want.insert((x + i, y)); want.insert((x + i, y + rh - 1)); }
                for j in 0..rh { want.insert((x, y + j)); want.insert((x + rw - 1, y + j)); }
            }
            prop_assert_eq!(lit_at(&f), on_screen(want, w, h));
        }

        #[test]
        fn a_filled_circle_is_every_point_within_r_plus_a_half((w, h) in size(), cx in coord(), cy in coord(), r in 0i64..30) {
            let mut f = fb(w, h);
            f.circ(cx, cy, r, 1, true);
            let want: Px = (cy - r..=cy + r).flat_map(|y| (cx - r..=cx + r).map(move |x| (x, y)))
                .filter(|&(x, y)| (x - cx).pow(2) + (y - cy).pow(2) <= r * r + r).collect();
            prop_assert_eq!(lit_at(&f), on_screen(want, w, h));
        }

        #[test]
        fn a_circle_outline_is_eightfold_symmetric_and_hugs_its_radius(r in 0i64..18) {
            let n = (2 * r + 3) as usize;
            let mut f = fb(n, n);
            let c = r + 1;
            f.circ(c, c, r, 1, false);
            let p = lit_at(&f);
            for &(x, y) in &p {
                let (dx, dy) = (x - c, y - c);
                for (sx, sy) in [(dx, dy), (-dx, dy), (dx, -dy), (-dx, -dy), (dy, dx), (-dy, dx), (dy, -dx), (-dy, -dx)] {
                    prop_assert!(p.contains(&(c + sx, c + sy)));
                }
                let d = ((dx * dx + dy * dy) as f64).sqrt();
                prop_assert!((d - r as f64).abs() < 1.0, "({dx},{dy}) at {d} for r {r}");
            }
        }

        #[test]
        fn a_filled_triangle_is_every_pixel_inside_its_edges((w, h) in size(), v in proptest::array::uniform6(-20i64..70)) {
            check_filled_tri(w, h, v)?;
            check_filled_tri(2, 13, [16, 30, 26, 42, 11, 24])?;
        }
    }
}

mod extremes {
    use proptest::prelude::*;
    use super::fb;

    fn any_coord() -> impl Strategy<Value = i64> {
        prop_oneof![-50i64..50, Just(i64::MIN), Just(i64::MAX), Just(i64::MIN / 2), Just(i64::MAX / 2), any::<i64>()]
            .prop_map(|v| posara_core::abi::Args(&[myriad::Value::from_int(v)]).coord(0))
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: ProptestConfig::default().cases.min(64), ..ProptestConfig::default() })]

        #[test]
        fn drawing_with_any_decoded_coordinates_finishes_and_stays_in_bounds(v in proptest::array::uniform6(any_coord()), t in 1i64..4, a in 0i64..300) {
            let [x0, y0, x1, y1, x2, y2] = v;
            let started = std::time::Instant::now();
            let mut f = fb(16, 12);
            f.line(x0, y0, x1, y1, 1);
            f.line_thick(x0, y0, x1, y1, t, 2);
            f.rect_outline(x0, y0, x1, y1, 3);
            f.circ(x0, y0, x1.abs(), 4, false);
            f.circ(x0, y0, x1.abs(), 5, true);
            f.tri(x0, y0, x1, y1, x2, y2, 6, true);
            f.rect_mix(x0, y0, x1, y1, 7, a);
            f.blit_4bpp(|_| 0x11, 0, x0, y0, x1, y1, 100, 256);
            f.blit_4bpp(|_| 0x11, 0, x0, y0, x1, y1, t * 37, a);
            prop_assert_eq!(f.buf.len(), 16 * 12);
            prop_assert!(started.elapsed().as_millis() < 500, "{:?} took {:?}", v, started.elapsed());
        }
    }
}

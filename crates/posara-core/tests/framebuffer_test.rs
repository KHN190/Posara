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

mod device_numerics {
    use std::collections::HashMap;

    use posara_core::gfx::{blitr_map, circle_span};
    use proptest::prelude::*;

    fn map<R: posara_core::math::Real>(w: i64, h: i64, deg: i64) -> HashMap<(i64, i64), (i64, i64)> {
        let mut m = HashMap::new();
        blitr_map::<R>(w, h, deg, |dx, dy, sx, sy| { m.insert((dx, dy), (sx, sy)); });
        m
    }

    proptest! {
        #[test]
        fn rotated_sprites_land_within_a_pixel_of_the_desktop_path(w in 1i64..96, h in 1i64..96, deg in -720i64..720) {
            let (host, device) = (map::<f64>(w, h, deg), map::<f32>(w, h, deg));
            let mut moved = 0;
            for (dest, &(hx, hy)) in &host {
                match device.get(dest) {
                    Some(&(dx, dy)) => {
                        prop_assert!(hx.abs_diff(dx) <= 1 && hy.abs_diff(dy) <= 1, "{dest:?}: {:?} vs {:?}", (hx, hy), (dx, dy));
                        if (hx, hy) != (dx, dy) { moved += 1; }
                    }
                    None => moved += 1,
                }
            }
            moved += device.keys().filter(|k| !host.contains_key(k)).count();
            prop_assert!(moved <= 1 || moved * 100 <= host.len(), "{moved} of {} pixels moved", host.len());
        }

        #[test]
        fn multiples_of_30_and_45_degrees_rotate_exactly_alike(w in 1i64..96, h in 1i64..96, k in -24i64..24, step in prop::sample::select(vec![30i64, 45])) {
            prop_assert_eq!(map::<f32>(w, h, k * step), map::<f64>(w, h, k * step));
        }

        #[test]
        fn filled_circle_spans_match_the_desktop_path(r in 0i64..4096, t in 0.0f64..=1.0) {
            let dy = (r as f64 * t) as i64;
            prop_assert_eq!(circle_span::<f32>(r, dy), circle_span::<f64>(r, dy));
        }
    }
}

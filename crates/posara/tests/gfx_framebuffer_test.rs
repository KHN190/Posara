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

    fn blend(d: u16, s: u16, a: u32) -> u16 {
        let ch = |v: u16, sh: u16, m: u16| ((v >> sh) & m) as u32;
        let mix = |sh: u16, m: u16| (ch(d, sh, m) * (256 - a) + ch(s, sh, m) * a) / 256;
        ((mix(11, 0x1F) << 11) | (mix(5, 0x3F) << 5) | mix(0, 0x1F)) as u16
    }

    fn put(m: &mut [u16], w: usize, h: usize, x: i64, y: i64, f: impl FnOnce(u16) -> u16) {
        if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
            let i = y as usize * w + x as usize;
            m[i] = f(m[i]);
        }
    }

    fn nibble(data: &[u8], n: usize) -> usize {
        let b = data.get(n / 2).copied().unwrap_or(0);
        (if n % 2 == 0 { b >> 4 } else { b & 0xF }) as usize
    }

    #[derive(Debug, Clone)]
    enum Op { Pset(i64, i64, u16), PsetOp(i64, i64, u16, i64), PsetA(i64, i64, u16, i64), Mix(i64, i64, i64, i64, u16, i64), Cls(u16) }

    fn op() -> impl Strategy<Value = Op> {
        let c = -12i64..40;
        prop_oneof![
            (c.clone(), c.clone(), any::<u16>()).prop_map(|(x, y, k)| Op::Pset(x, y, k)),
            (c.clone(), c.clone(), any::<u16>(), -1i64..6).prop_map(|(x, y, k, m)| Op::PsetOp(x, y, k, m)),
            (c.clone(), c.clone(), any::<u16>(), -40i64..400).prop_map(|(x, y, k, a)| Op::PsetA(x, y, k, a)),
            (c.clone(), c.clone(), -4i64..40, -4i64..40, any::<u16>(), -40i64..400).prop_map(|(x, y, w, h, k, a)| Op::Mix(x, y, w, h, k, a)),
            any::<u16>().prop_map(Op::Cls),
        ]
    }

    proptest! {
        #[test]
        fn pixel_ops_match_a_per_pixel_model(w in 1usize..32, h in 1usize..32, ops in proptest::collection::vec(op(), 0..60)) {
            let mut f = fb(w, h);
            let mut m = vec![0u16; w * h];
            for o in &ops {
                match *o {
                    Op::Pset(x, y, k) => { f.pset(x, y, k); put(&mut m, w, h, x, y, |_| k); }
                    Op::PsetOp(x, y, k, mode) => {
                        f.pset_op(x, y, k, mode);
                        put(&mut m, w, h, x, y, |d| match mode { 1 => d ^ k, 2 => d & k, 3 => d | k, _ => k });
                    }
                    Op::PsetA(x, y, k, a) => { f.pset_a(x, y, k, a); put(&mut m, w, h, x, y, |d| blend(d, k, a.clamp(0, 256) as u32)); }
                    Op::Mix(x, y, rw, rh, k, a) => {
                        f.rect_mix(x, y, rw, rh, k, a);
                        let a = a.clamp(0, 256) as u32;
                        if a > 0 {
                            for j in 0..rh.max(0) { for i in 0..rw.max(0) { put(&mut m, w, h, x + i, y + j, |d| blend(d, k, a)); } }
                        }
                    }
                    Op::Cls(k) => { f.cls(k); m.iter_mut().for_each(|p| *p = k); }
                }
            }
            prop_assert_eq!(&f.buf, &m);
        }

        #[test]
        fn a_4bpp_blit_matches_nearest_neighbour_sampling(
            w in 1usize..24, h in 1usize..24, x0 in -20i64..30, y0 in -20i64..30,
            sw in 1i64..12, sh in 1i64..12, pct in prop_oneof![Just(100i64), 1i64..400], alpha in prop_oneof![Just(256i64), 0i64..256],
            off in 0usize..8, data in proptest::collection::vec(any::<u8>(), 0..80), pal in proptest::array::uniform16(any::<u16>()),
        ) {
            let mut f = fb(w, h);
            f.cls(0x5555);
            f.palette = pal;
            f.blit_4bpp(|i| data.get(i).copied().unwrap_or(0), off, x0, y0, sw, sh, pct, alpha);
            let mut m = vec![0x5555u16; w * h];
            let (dw, dh) = ((sw * pct / 100).max(1), (sh * pct / 100).max(1));
            for dy in 0..dh {
                for dx in 0..dw {
                    let idx = nibble(&data, off + ((dy * 100 / pct) * sw + dx * 100 / pct) as usize);
                    if idx == 0 { continue; }
                    let k = pal[idx];
                    put(&mut m, w, h, x0 + dx, y0 + dy, |d| if alpha >= 256 { k } else { blend(d, k, alpha.clamp(0, 256) as u32) });
                }
            }
            prop_assert_eq!(&f.buf, &m);
        }

        #[test]
        fn a_screen_fits_only_within_its_budget(w in 0usize..600, h in 0usize..600, cap in 1usize..600, format in 0u8..3) {
            let mut f = posara::Framebuffer::new();
            f.set_headless();
            f.max_dim = cap;
            let ok = format == 1 && w > 0 && h > 0 && w <= cap && h <= cap;
            prop_assert_eq!(f.configure(w, h, format).is_ok(), ok);
            if ok {
                prop_assert_eq!(f.buf.len(), w * h);
                prop_assert!(f.configure(w, h, 1).is_ok());
                prop_assert!(f.configure(w + 1, h, 1).is_err() || w + 1 > cap);
            }
        }

        #[test]
        fn dithering_leaves_only_the_two_inks_and_keeps_solid_extremes(w in 1usize..24, h in 1usize..24, px in proptest::collection::vec(any::<u16>(), 576), dark in any::<u16>(), light in any::<u16>()) {
            let mut f = fb(w, h);
            f.buf.copy_from_slice(&px[..w * h]);
            f.dither(dark, light);
            prop_assert!(f.buf.iter().all(|&p| p == dark || p == light));
            f.cls(0x0000);
            f.dither(dark, light);
            prop_assert!(f.buf.iter().all(|&p| p == dark));
            f.cls(0xFFFF);
            f.dither(dark, light);
            prop_assert!(f.buf.iter().all(|&p| p == light));
        }
    }

    #[test]
    fn every_rgb565_expands_to_full_range_rgba() {
        for c in 0..=u16::MAX {
            let (r, g, b) = ((c >> 11) as u32, ((c >> 5) & 63) as u32, (c & 31) as u32);
            let want = 0xFF00_0000 | (b * 255 / 31) << 16 | (g * 255 / 63) << 8 | r * 255 / 31;
            assert_eq!(posara_core::gfx::framebuffer::rgb565_to_rgba(c), want, "{c:#06x}");
        }
    }
}

#[cfg(feature = "compiler")]
mod natives {
    use proptest::prelude::*;

    fn frame_of(w: usize, h: usize, body: &str) -> Vec<u16> {
        use posara::runner::{compile_source, Stepper};
        let src = format!("@cart\nfn main() -> <frame, Graphics, IO> Unit {{\n  gfx_screen({w}, {h});\n  gfx_cls(0);\n  {body};\n  gfx_commit();\n  loop {{ frame.present() }}\n}}\n");
        let host = posara::Host::new_with(std::path::PathBuf::from("."), true, true).unwrap();
        let r = compile_source(&src, &host).unwrap_or_else(|e| panic!("{e}\n{src}"));
        Stepper::start_named(r.module, r.static_names, r.fn_names, &host).unwrap();
        let buf = host.gfx.fb.borrow().buf.clone();
        buf
    }

    fn bytes_literal(d: &[u8]) -> String {
        let mut s = String::from("b\"");
        for b in d { s.push_str(&format!("\\x{b:02x}")); }
        s.push('"');
        s
    }

    fn put(m: &mut [u16], w: usize, h: usize, x: i64, y: i64, f: impl FnOnce(u16) -> u16) {
        if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
            let i = y as usize * w + x as usize;
            m[i] = f(m[i]);
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: ProptestConfig::default().cases.min(64), ..ProptestConfig::default() })]

        #[test]
        fn gfx_rect_fills_its_on_screen_part(w in 1usize..24, h in 1usize..24, x in -30i64..30, y in -30i64..30, rw in -5i64..40, rh in -5i64..40, c in 1u16..) {
            let buf = frame_of(w, h, &format!("gfx_rect({x}, {y}, {rw}, {rh}, {c})"));
            let mut m = vec![0u16; w * h];
            for j in 0..rh.max(0) { for i in 0..rw.max(0) { put(&mut m, w, h, x + i, y + j, |_| c); } }
            prop_assert_eq!(buf, m);
        }

        #[test]
        fn gfx_blitg_rotates_and_composites_each_set_bit(
            w in 1usize..20, h in 1usize..20, x in -8i64..20, y in -8i64..20, sw in 1i64..10, sh in 1i64..10,
            off in 0i64..12, rot in 0i64..4, mode in 0i64..4, c in 1u16.., data in proptest::collection::vec(any::<u8>(), 1..24),
        ) {
            let body = format!("let s = {};\n  gfx_rect(0, 0, {w}, {h}, 0x0F0F);\n  gfx_blitg(&s, {off}, {x}, {y}, {sw}, {sh}, {c}, {})", bytes_literal(&data), mode | rot << 4);
            let buf = frame_of(w, h, &body);
            let mut m = vec![0x0F0Fu16; w * h];
            for py in 0..sh {
                for px in 0..sw {
                    let bit = (off + py * sw + px) as usize;
                    let set = data.get(bit / 8).is_some_and(|b| b >> (7 - bit % 8) & 1 == 1);
                    if !set { continue; }
                    let (dx, dy) = match rot { 1 => (sh - 1 - py, px), 2 => (sw - 1 - px, sh - 1 - py), 3 => (py, sw - 1 - px), _ => (px, py) };
                    put(&mut m, w, h, x + dx, y + dy, |d| match mode { 1 => d ^ c, 2 => d & c, 3 => d | c, _ => c });
                }
            }
            prop_assert_eq!(buf, m);
        }

        #[test]
        fn gfx_pal_keeps_the_top_bits_of_each_channel(i in 0i64..16, rgb in 0i64..0x1000000) {
            let (r, g, b) = ((rgb >> 16) as u16, (rgb >> 8 & 0xFF) as u16, (rgb & 0xFF) as u16);
            let want = (r >> 3) << 11 | (g >> 2) << 5 | b >> 3;
            let idx = i.clamp(1, 15);
            let byte = (idx << 4) as u8;
            let body = format!("gfx_pal({idx}, {rgb});\n  let s = {};\n  gfx_sprite(&s, 0, 0, 0, 1, 1, 100, 256)", bytes_literal(&[byte]));
            prop_assert_eq!(frame_of(1, 1, &body)[0], want);
        }
    }

    #[test]
    fn a_huge_rect_only_touches_the_screen() {
        let t = std::time::Instant::now();
        let buf = frame_of(4, 3, "gfx_rect(-1000000, 1, 2000000, 1, 7)");
        assert!(t.elapsed().as_secs() < 2, "rect walked off-screen pixels");
        assert_eq!(buf, [0, 0, 0, 0, 7, 7, 7, 7, 0, 0, 0, 0]);
    }
}

pub mod framebuffer;
mod device;
mod raster;
pub mod input;

pub use device::{ScreenDevice, SCREEN_ID};
pub use framebuffer::Framebuffer;

use core::cell::RefCell;

use myriad::{NativeCtx, Value, VirtualMachine};
use crate::abi::{Args, COORD_LIM};


const fn sin_q16(d: usize) -> i32 {
    let (q, neg) = if d < 180 { (d, false) } else { (d - 180, true) };
    let q = if q > 90 { 180 - q } else { q };
    let x = q as f64 * core::f64::consts::PI / 180.0;
    let (mut term, mut sum, mut k) = (x, x, 1);
    while k < 12 {
        term = -term * x * x / ((2 * k) as f64 * (2 * k + 1) as f64);
        sum += term;
        k += 1;
    }
    let v = (sum * 65536.0 + 0.5) as i32;
    if neg { -v } else { v }
}

pub const SIN_Q16: [i32; 360] = {
    let mut t = [0; 360];
    let mut d = 0;
    while d < 360 {
        t[d] = sin_q16(d);
        d += 1;
    }
    t
};

const ROTATE_MAX: i64 = 4096;

// Destination pixel centres map back to source coordinates in Q16 and floor: integer only, so every platform agrees.
pub fn blitr_map(w: i64, h: i64, deg: i64, put: impl FnMut(i64, i64, i64, i64)) {
    blitr_map_in(w, h, deg, i64::MIN..i64::MAX, i64::MIN..i64::MAX, put)
}

pub fn blitr_map_in(w: i64, h: i64, deg: i64, xs: core::ops::Range<i64>, ys: core::ops::Range<i64>, mut put: impl FnMut(i64, i64, i64, i64)) {
    if !(1..=ROTATE_MAX).contains(&w) || !(1..=ROTATE_MAX).contains(&h) { return; }
    let d = deg.rem_euclid(360) as usize;
    let (s, c) = (SIN_Q16[d] as i64, SIN_Q16[(d + 90) % 360] as i64);
    let half = (crate::math::isqrt_ceil(w * w + h * h) + 1) / 2 + 1;
    let centre = |n: i64| if n % 2 == 0 { 1 << 15 } else { 0 };
    for ddy in (-half).max(ys.start)..=half.min(ys.end - 1) {
        for ddx in (-half).max(xs.start)..=half.min(xs.end - 1) {
            let (ox, oy) = ((ddx << 16) + centre(w), (ddy << 16) + centre(h));
            let sx = (c * ox + s * oy + (w << 31)) >> 32;
            let sy = (-s * ox + c * oy + (h << 31)) >> 32;
            if (0..w).contains(&sx) && (0..h).contains(&sy) { put(ddx, ddy, sx, sy); }
        }
    }
}

pub fn circle_span(r: i64, dy: i64) -> i64 { crate::math::isqrt(r * r + r - dy * dy) }

use crate::drivers::Presenter;
use crate::plugin::Plugin;
use crate::prelude::*;

pub struct GfxPlugin {
    pub fb: Rc<RefCell<Framebuffer>>,
}

impl GfxPlugin {
    pub fn new(presenter: Option<Box<dyn Presenter>>) -> Self {
        let mut fb = Framebuffer::new();
        if let Some(p) = presenter { fb.set_presenter(p); }
        Self { fb: Rc::new(RefCell::new(fb)) }
    }
}

impl Plugin for GfxPlugin {
    fn install(&self, vm: &mut VirtualMachine) {
        vm.install_device(SCREEN_ID, Box::new(ScreenDevice::new(Rc::clone(&self.fb))));
        register_natives(vm, Rc::clone(&self.fb));
    }
}

fn ret_unit() -> Result<(Value, bool), String> { Ok((Value::UNIT, false)) }

// Is the `bit`-th pixel of a 1bpp sprite set? MSB-first within each byte.
fn bit1(bytes: &[u8], bit: usize) -> bool {
    let byte = bytes.get(bit / 8).copied().unwrap_or(0);
    (byte >> (7 - (bit & 7))) & 1 == 1
}

pub fn register_natives(vm: &mut VirtualMachine, fb: Rc<RefCell<Framebuffer>>) -> Vec<&'static str> {
    let mut names = Vec::new();
    macro_rules! reg {
        ($name:literal, $body:expr) => {{
            vm.register_native($name, Rc::new($body) as myriad::NativeFn);
            names.push($name);
        }};
    }
    let f = Rc::clone(&fb);
    reg!("gfx_screen", move |_: &mut NativeCtx, a: &[Value]| {
        let a = Args(a);
        f.borrow_mut().configure(a.within(0, 0, COORD_LIM) as usize, a.within(1, 0, COORD_LIM) as usize, 1)?;
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_screen_off", move |_: &mut NativeCtx, _a: &[Value]| {
        f.borrow_mut().set_headless();
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_commit", move |_: &mut NativeCtx, _a: &[Value]| {
        f.borrow_mut().commit()?;
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_cls", move |_: &mut NativeCtx, a: &[Value]| {
        let a = Args(a);
        f.borrow_mut().cls(a.color(0));
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_win_pos", move |_: &mut NativeCtx, a: &[Value]| {
        let a = Args(a);
        f.borrow_mut().win_pos(a.coord(0), a.coord(1));
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_rectmix", move |_: &mut NativeCtx, a: &[Value]| {
        let a = Args(a);
        f.borrow_mut().rect_mix(a.coord(0), a.coord(1), a.coord(2), a.coord(3), a.color(4), a.within(5, 0, 256));
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_dither", move |_: &mut NativeCtx, a: &[Value]| {
        let a = Args(a);
        f.borrow_mut().dither(a.color(0), a.color(1));
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_pset", move |_: &mut NativeCtx, a: &[Value]| {
        let a = Args(a);
        f.borrow_mut().pset(a.coord(0), a.coord(1), a.color(2));
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_rect", move |_: &mut NativeCtx, a: &[Value]| {
        let a = Args(a);
        let (x0, y0, w, h, c) = (a.coord(0), a.coord(1), a.coord(2), a.coord(3), a.color(4));
        let mut fbm = f.borrow_mut();
        let (fw, fh) = (fbm.w, fbm.h);
        for y in framebuffer::span(y0, h, fh) {
            for x in framebuffer::span(x0, w, fw) { fbm.buf[y * fw + x] = c; }
        }
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_rectb", move |_: &mut NativeCtx, a: &[Value]| {
        let a = Args(a);
        f.borrow_mut().rect_outline(a.coord(0), a.coord(1), a.coord(2), a.coord(3), a.color(4));
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_line", move |_: &mut NativeCtx, a: &[Value]| {
        let a = Args(a);
        f.borrow_mut().line(a.coord(0), a.coord(1), a.coord(2), a.coord(3), a.color(4));
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_linew", move |_: &mut NativeCtx, a: &[Value]| {
        let a = Args(a);
        f.borrow_mut().line_thick(a.coord(0), a.coord(1), a.coord(2), a.coord(3), a.coord(4), a.color(5));
        ret_unit()
    });
    // blitr(sprite, x, y, w, h, color, mode, deg): arbitrary-angle rotation.
    // Rotates about the sprite center (kept at the same place as the unrotated
    // x,y,w,h), reverse-sampling each dest pixel from the source (nearest). Slow
    // vs blitg's 90-deg shuffle; use for the occasional tilted element. Reads
    // from the sprite start (no atlas offset — native arg cap is 8).
    let f = Rc::clone(&fb);
    reg!("gfx_blitr", move |ctx: &mut NativeCtx, a: &[Value]| {
        let a = Args(a);
        let sprite = a.handle(0);
        let off = 0usize;
        let (x0, y0, w, h, color) = (a.coord(1), a.coord(2), a.coord(3), a.coord(4), a.color(5));
        let mode = a.word(6) & 0xF;
        let deg = a.degrees(7);
        if sprite.is_handle_none() || w <= 0 || h <= 0 { return ret_unit(); }
        let Some(bytes) = myriad::read_bytes(ctx.heap, sprite) else { return ret_unit(); };
        let (cxd, cyd) = (x0 + w / 2, y0 + h / 2);
        let mut fbm = f.borrow_mut();
        let (fw, fh) = (fbm.w as i64, fbm.h as i64);
        blitr_map_in(w, h, deg, -cxd..fw - cxd, -cyd..fh - cyd, |ddx, ddy, sx, sy| {
            if bit1(&bytes, off + (sy * w + sx) as usize) { fbm.pset_op(cxd + ddx, cyd + ddy, color, mode); }
        });
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_circ", move |_: &mut NativeCtx, a: &[Value]| {
        let a = Args(a);
        f.borrow_mut().circ(a.coord(0), a.coord(1), a.coord(2), a.color(3), true);
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_circb", move |_: &mut NativeCtx, a: &[Value]| {
        let a = Args(a);
        f.borrow_mut().circ(a.coord(0), a.coord(1), a.coord(2), a.color(3), false);
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_tri", move |_: &mut NativeCtx, a: &[Value]| {
        let a = Args(a);
        f.borrow_mut().tri(a.coord(0), a.coord(1), a.coord(2), a.coord(3), a.coord(4), a.coord(5), a.color(6), true);
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_trib", move |_: &mut NativeCtx, a: &[Value]| {
        let a = Args(a);
        f.borrow_mut().tri(a.coord(0), a.coord(1), a.coord(2), a.coord(3), a.coord(4), a.coord(5), a.color(6), false);
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_pal", move |_: &mut NativeCtx, a: &[Value]| {
        let a = Args(a);
        let i = a.within(0, 0, 15) as usize;
        let rgb = a.word(1) as u32;
        let r = ((rgb >> 16) & 0xFF) as u8;
        let g = ((rgb >> 8)  & 0xFF) as u8;
        let b = ( rgb        & 0xFF) as u8;
        let r5 = (r as u16 >> 3) & 0x1F;
        let g6 = (g as u16 >> 2) & 0x3F;
        let b5 = (b as u16 >> 3) & 0x1F;
        f.borrow_mut().palette[i] = (r5 << 11) | (g6 << 5) | b5;
        ret_unit()
    });
    // blitg(sprite, off_bits, x, y, w, h, color, mode): 1bpp blit reading from a
    // bit offset into the packed sprite (so one atlas array holds many glyphs),
    // with composite mode 0 REPLACE / 1 XOR / 2 AND / 3 OR. rot in high bits of
    // mode: bits 4..6 = 0/1/2/3 → 0/90/180/270 deg (sprite source rotation).
    let f = Rc::clone(&fb);
    reg!("gfx_blitg", move |ctx: &mut NativeCtx, a: &[Value]| {
        let a = Args(a);
        let sprite = a.handle(0);
        let off = a.index(1);
        let (x0, y0, w, h, color) = (a.coord(2), a.coord(3), a.coord(4), a.coord(5), a.color(6));
        let modeword = a.word(7);
        let mode = modeword & 0xF;
        let rot = (modeword >> 4) & 0x3;
        if sprite.is_handle_none() { return ret_unit(); }
        let Some(bytes) = myriad::read_bytes(ctx.heap, sprite) else { return ret_unit(); };
        if w <= 0 || h <= 0 { return ret_unit(); }
        let mut fbm = f.borrow_mut();
        let bits = bytes.len() as i64 * 8;
        for py in 0..h {
            let row = off as i64 + py * w;
            if row >= bits { break; }
            for px in 0..w.min(bits - row) {
                let bit = (row + px) as usize;
                if bit1(&bytes, bit) {
                    let (dx, dy) = match rot {
                        1 => (h - 1 - py, px),       // 90 cw
                        2 => (w - 1 - px, h - 1 - py),
                        3 => (py, w - 1 - px),       // 270 cw
                        _ => (px, py),
                    };
                    fbm.pset_op(x0 + dx, y0 + dy, color, mode);
                }
            }
        }
        ret_unit()
    });
    let f = Rc::clone(&fb);
    // sprite(data, off, x, y, w, h, scale, alpha): 4bpp palette blit. alpha 256 =
    // opaque (fast pset path); alpha < 256 alpha-blends each pixel over the
    // framebuffer, for crossfading pre-rendered frames. index 0 transparent.
    reg!("gfx_sprite", move |ctx: &mut NativeCtx, a: &[Value]| {
        let a = Args(a);
        let data = a.handle(0);
        let off = a.index(1);
        let (x0, y0, w, h) = (a.coord(2), a.coord(3), a.coord(4), a.coord(5));
        let pct = if a.0.len() > 6 { a.within(6, 1, 10_000) } else { 100 };
        let alpha = if a.0.len() > 7 { a.within(7, 0, 256) } else { 256 };
        if data.is_handle_none() || w <= 0 || h <= 0 { return ret_unit(); }
        let Some(bytes) = myriad::read_bytes(ctx.heap, data) else { return ret_unit(); };
        let mut fbm = f.borrow_mut();
        fbm.blit_4bpp(|i| bytes.get(i).copied().unwrap_or(0), off, x0, y0, w, h, pct, alpha);
        ret_unit()
    });
    names
}


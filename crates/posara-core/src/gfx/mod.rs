pub mod framebuffer;
mod device;
mod raster;
pub mod input;

pub use device::{ScreenDevice, SCREEN_ID};
pub use framebuffer::Framebuffer;

use core::cell::RefCell;

use myriad::{NativeCtx, Value, VirtualMachine};


// Desktop/web keep f64 (recorded output); devices use the FPU's f32.
#[cfg(feature = "std")]
pub(crate) type Real = f64;
#[cfg(not(feature = "std"))]
pub(crate) type Real = f32;

pub fn blitr_map<R: crate::math::Real>(w: i64, h: i64, deg: i64, mut put: impl FnMut(i64, i64, i64, i64)) {
    let two = R::of(2);
    let (cx, cy) = (R::of(w) / two, R::of(h) / two);
    let (s, c) = exact_sin_cos::<R>(deg).unwrap_or_else(|| (R::of(deg) * R::PI / R::of(180)).sin_cos());
    let half = (R::of(w * w + h * h).sqrt() / two).ceil().int() + 1;
    for ddy in -half..=half {
        for ddx in -half..=half {
            let sx = (c * R::of(ddx) + s * R::of(ddy) + cx).round().int();
            let sy = (-s * R::of(ddx) + c * R::of(ddy) + cy).round().int();
            if (0..w).contains(&sx) && (0..h).contains(&sy) { put(ddx, ddy, sx, sy); }
        }
    }
}

// Multiples of 30° and 45°: ½ and |sin| = |cos| must be exact or x.5 ties round apart between f32 and f64.
fn exact_sin_cos<R: crate::math::Real>(deg: i64) -> Option<(R, R)> {
    let d = deg.rem_euclid(360);
    let two = R::of(2);
    let (half, r2, r3) = (R::of(1) / two, R::of(2).sqrt() / two, R::of(3).sqrt() / two);
    if d % 45 == 0 {
        let sine = |k: i64| [R::of(0), r2, R::of(1), r2, R::of(0), -r2, R::of(-1), -r2][k.rem_euclid(8) as usize];
        return Some((sine(d / 45), sine(d / 45 + 2)));
    }
    if d % 30 != 0 { return None; }
    let sine = |k: i64| [R::of(0), half, r3, R::of(1), r3, half, R::of(0), -half, -r3, R::of(-1), -r3, -half][k.rem_euclid(12) as usize];
    Some((sine(d / 30), sine(d / 30 + 3)))
}

pub fn circle_span<R: crate::math::Real>(r: i64, dy: i64) -> i64 { R::of(r * r + r - dy * dy).sqrt().int() }
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

fn arg(args: &[Value], i: usize) -> i64 {
    args.get(i).copied().unwrap_or(Value::ZERO).as_int()
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
        f.borrow_mut().configure(arg(a, 0) as usize, arg(a, 1) as usize, 1)?;
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
        f.borrow_mut().cls(arg(a, 0) as u16);
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_win_pos", move |_: &mut NativeCtx, a: &[Value]| {
        f.borrow_mut().win_pos(arg(a, 0), arg(a, 1));
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_rectmix", move |_: &mut NativeCtx, a: &[Value]| {
        f.borrow_mut().rect_mix(arg(a, 0), arg(a, 1), arg(a, 2), arg(a, 3), arg(a, 4) as u16, arg(a, 5));
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_dither", move |_: &mut NativeCtx, a: &[Value]| {
        f.borrow_mut().dither(arg(a, 0) as u16, arg(a, 1) as u16);
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_pset", move |_: &mut NativeCtx, a: &[Value]| {
        f.borrow_mut().pset(arg(a, 0), arg(a, 1), arg(a, 2) as u16);
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_rect", move |_: &mut NativeCtx, a: &[Value]| {
        let (x0, y0, w, h, c) = (arg(a, 0), arg(a, 1), arg(a, 2), arg(a, 3), arg(a, 4) as u16);
        let mut fbm = f.borrow_mut();
        let (fw, fh) = (fbm.w, fbm.h);
        for y in framebuffer::span(y0, h, fh) {
            for x in framebuffer::span(x0, w, fw) { fbm.buf[y * fw + x] = c; }
        }
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_rectb", move |_: &mut NativeCtx, a: &[Value]| {
        f.borrow_mut().rect_outline(arg(a, 0), arg(a, 1), arg(a, 2), arg(a, 3), arg(a, 4) as u16);
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_line", move |_: &mut NativeCtx, a: &[Value]| {
        f.borrow_mut().line(arg(a, 0), arg(a, 1), arg(a, 2), arg(a, 3), arg(a, 4) as u16);
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_linew", move |_: &mut NativeCtx, a: &[Value]| {
        f.borrow_mut().line_thick(arg(a, 0), arg(a, 1), arg(a, 2), arg(a, 3), arg(a, 4), arg(a, 5) as u16);
        ret_unit()
    });
    // blitr(sprite, x, y, w, h, color, mode, deg): arbitrary-angle rotation.
    // Rotates about the sprite center (kept at the same place as the unrotated
    // x,y,w,h), reverse-sampling each dest pixel from the source (nearest). Slow
    // vs blitg's 90-deg shuffle; use for the occasional tilted element. Reads
    // from the sprite start (no atlas offset — native arg cap is 8).
    let f = Rc::clone(&fb);
    reg!("gfx_blitr", move |ctx: &mut NativeCtx, a: &[Value]| {
        let sprite = a.first().copied().unwrap_or(Value::NONE);
        let off = 0usize;
        let (x0, y0, w, h, color) = (arg(a, 1), arg(a, 2), arg(a, 3), arg(a, 4), arg(a, 5) as u16);
        let mode = arg(a, 6) & 0xF;
        let deg = arg(a, 7);
        if sprite.is_handle_none() || w <= 0 || h <= 0 { return ret_unit(); }
        let Some(bytes) = myriad::read_bytes(ctx.heap, sprite) else { return ret_unit(); };
        let (cxd, cyd) = (x0 + w / 2, y0 + h / 2);
        let mut fbm = f.borrow_mut();
        blitr_map::<Real>(w, h, deg, |ddx, ddy, sx, sy| {
            if bit1(&bytes, off + (sy * w + sx) as usize) { fbm.pset_op(cxd + ddx, cyd + ddy, color, mode); }
        });
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_circ", move |_: &mut NativeCtx, a: &[Value]| {
        f.borrow_mut().circ(arg(a, 0), arg(a, 1), arg(a, 2), arg(a, 3) as u16, true);
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_circb", move |_: &mut NativeCtx, a: &[Value]| {
        f.borrow_mut().circ(arg(a, 0), arg(a, 1), arg(a, 2), arg(a, 3) as u16, false);
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_tri", move |_: &mut NativeCtx, a: &[Value]| {
        f.borrow_mut().tri(arg(a, 0), arg(a, 1), arg(a, 2), arg(a, 3), arg(a, 4), arg(a, 5), arg(a, 6) as u16, true);
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_trib", move |_: &mut NativeCtx, a: &[Value]| {
        f.borrow_mut().tri(arg(a, 0), arg(a, 1), arg(a, 2), arg(a, 3), arg(a, 4), arg(a, 5), arg(a, 6) as u16, false);
        ret_unit()
    });
    let f = Rc::clone(&fb);
    reg!("gfx_pal", move |_: &mut NativeCtx, a: &[Value]| {
        let i = arg(a, 0).clamp(0, 15) as usize;
        let rgb = arg(a, 1) as u32;
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
        let sprite = a.first().copied().unwrap_or(Value::NONE);
        let off = arg(a, 1).max(0) as usize;
        let (x0, y0, w, h, color) = (arg(a, 2), arg(a, 3), arg(a, 4), arg(a, 5), arg(a, 6) as u16);
        let modeword = arg(a, 7);
        let mode = modeword & 0xF;
        let rot = (modeword >> 4) & 0x3;
        if sprite.is_handle_none() { return ret_unit(); }
        let Some(bytes) = myriad::read_bytes(ctx.heap, sprite) else { return ret_unit(); };
        let mut fbm = f.borrow_mut();
        let bits = bytes.len() as i128 * 8;
        for py in 0..h {
            let row = off as i128 + py as i128 * w as i128;
            if row >= bits { break; }
            for px in 0..(w as i128).min(bits - row) as i64 {
                let bit = (row + px as i128) as usize;
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
        let data = a.first().copied().unwrap_or(Value::NONE);
        let off = arg(a, 1).max(0) as usize;
        let (x0, y0, w, h) = (arg(a, 2), arg(a, 3), arg(a, 4), arg(a, 5));
        let pct = match a.get(6) { Some(v) => v.as_int().max(1), None => 100 };
        let alpha = match a.get(7) { Some(v) => v.as_int(), None => 256 };
        if data.is_handle_none() || w <= 0 || h <= 0 { return ret_unit(); }
        let Some(bytes) = myriad::read_bytes(ctx.heap, data) else { return ret_unit(); };
        let mut fbm = f.borrow_mut();
        fbm.blit_4bpp(|i| bytes.get(i).copied().unwrap_or(0), off, x0, y0, w, h, pct, alpha);
        ret_unit()
    });
    names
}


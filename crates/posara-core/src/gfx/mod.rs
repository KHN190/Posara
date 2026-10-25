pub mod framebuffer;
mod device;
mod raster;
pub mod input;

pub use device::{ScreenDevice, SCREEN_ID};
pub use framebuffer::Framebuffer;

use core::cell::RefCell;

use super::runtime::{self, NativeCtx, Value, VirtualMachine, Int};
use super::abi::{Args, COORD_LIM};


pub fn sin_q16(degrees: usize) -> i32 {
    let d = degrees % 360;
    let (q, negative) = if d < 180 { (d, false) } else { (d - 180, true) };
    let q = if q > 90 { 180 - q } else { q };
    let x = q as f32 * core::f32::consts::PI / 180.0;
    let value = (crate::math::sin(x) * 65536.0 + 0.5) as i32;
    if negative { -value } else { value }
}

const ROTATE_MAX: Int = 4096;

pub fn blitr_map(w: Int, h: Int, deg: Int, put: impl FnMut(Int, Int, Int, Int)) {
    blitr_map_in(w, h, deg, Int::MIN..Int::MAX, Int::MIN..Int::MAX, put)
}

pub fn blitr_map_in(w: Int, h: Int, deg: Int, xs: core::ops::Range<Int>, ys: core::ops::Range<Int>, mut put: impl FnMut(Int, Int, Int, Int)) {
    if !(1..=ROTATE_MAX).contains(&w) || !(1..=ROTATE_MAX).contains(&h) || xs.is_empty() || ys.is_empty() { return; }
    let d = deg.rem_euclid(360) as usize;
    let (s, c) = (sin_q16(d) as Int, sin_q16(d + 90) as Int);
    let half = (crate::math::isqrt_ceil(w * w + h * h) + 1) / 2 + 1;
    let (cw, ch) = (Int::from(w % 2 == 0), Int::from(h % 2 == 0));
    for ddy in (-half).max(ys.start)..=half.min(ys.end - 1) {
        for ddx in (-half).max(xs.start)..=half.min(xs.end - 1) {
            let sx = (2 * (c * ddx + s * ddy) + cw * c + ch * s + w * 65536) >> 17;
            let sy = (2 * (-s * ddx + c * ddy) - cw * s + ch * c + h * 65536) >> 17;
            if (0..w).contains(&sx) && (0..h).contains(&sy) { put(ddx, ddy, sx, sy); }
        }
    }
}

pub fn circle_span(r: Int, dy: Int) -> Int {
    if Int::BITS == 64 { return crate::math::isqrt(r * r + r - dy * dy); }
    if !(0..=COORD_LIM).contains(&r) || dy < -r || dy > r { return 0; }
    let (r,d) = (r as u32, dy.unsigned_abs() as u32);
    let estimate = (r-d) as f32 * (r+d) as f32 + r as f32;
    let mut x = myriad::word32::fmath::sqrt(estimate) as u32;
    // In the coordinate domain the true residual is below 2^23, so wrapping recovers its exact signed value.
    let mut residual = r.wrapping_mul(r).wrapping_add(r).wrapping_sub(d.wrapping_mul(d)).wrapping_sub(x.wrapping_mul(x)) as i32;
    while residual < 0 { residual += (2*x-1) as i32; x -= 1; }
    while residual >= (2*x+1) as i32 { residual -= (2*x+1) as i32; x += 1; }
    x as Int
}

use super::drivers::Presenter;
use super::plugin::Plugin;
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

fn blit_bits(fbm: &mut Framebuffer, bytes: &[u8], off: usize, x0: Int, y0: Int, w: Int, h: Int, color: u16, mode: Int, rot: Int) {
    for py in 0..h {
        let Some(row) = (py as usize).checked_mul(w as usize).and_then(|n| n.checked_add(off)) else { break; };
        if row / 8 >= bytes.len() { break; }
        let available = (bytes.len() - row / 8).saturating_mul(8).saturating_sub(row % 8);
        for px in 0..(w as usize).min(available) {
            let Some(bit) = row.checked_add(px) else { break; };
            let px = px as Int;
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
}

pub fn register_natives(vm: &mut VirtualMachine, fb: Rc<RefCell<Framebuffer>>) -> Vec<&'static str> {
    let mut names = Vec::new();
    macro_rules! reg {
        ($name:literal, $body:expr) => {{
            vm.register_native($name, Rc::new($body) as runtime::NativeFn);
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
        let Some(bytes) = runtime::read_bytes(ctx.heap, sprite) else { return ret_unit(); };
        let (cxd, cyd) = (x0 + w / 2, y0 + h / 2);
        let mut fbm = f.borrow_mut();
        let (fw, fh) = (fbm.w as Int, fbm.h as Int);
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
        let Some(bytes) = runtime::read_bytes(ctx.heap, sprite) else { return ret_unit(); };
        if w <= 0 || h <= 0 { return ret_unit(); }
        let mut fbm = f.borrow_mut();
        blit_bits(&mut fbm, &bytes, off, x0, y0, w, h, color, mode, rot);
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
        let Some(bytes) = runtime::read_bytes(ctx.heap, data) else { return ret_unit(); };
        let mut fbm = f.borrow_mut();
        fbm.blit_4bpp(|i| bytes.get(i).copied().unwrap_or(0), off, x0, y0, w, h, pct, alpha);
        ret_unit()
    });
    names
}


#[cfg(test)]
mod blit_tests {
    use super::*;
    #[test]
    fn huge_or_overflowing_offsets_remain_transparent() {
        let mut fb=Framebuffer::new(); fb.configure(4,4,1).unwrap();
        for off in [usize::MAX, Int::MAX as usize] {
            blit_bits(&mut fb,&[255],off,0,0,COORD_LIM,COORD_LIM,65535,0,0);
            assert!(fb.buf.iter().all(|&p|p==0));
        }
        blit_bits(&mut fb,&[255],0,-COORD_LIM+1,0,COORD_LIM,COORD_LIM,65535,0,0);
        assert!(fb.buf.iter().all(|&p|p==0));
        blit_bits(&mut fb,&[128],0,0,0,COORD_LIM,COORD_LIM,65535,0,0);
        assert_eq!(fb.buf.iter().filter(|&&p|p!=0).count(),1);
    }
}

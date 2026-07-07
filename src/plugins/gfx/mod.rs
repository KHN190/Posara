pub mod framebuffer;
mod device;
mod png;
mod raster;

pub use device::{ScreenDevice, SCREEN_ID};
pub use framebuffer::Framebuffer;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use myriad::{read_string, NativeCtx, Value, VirtualMachine};

pub mod input;

use input::{Controller, ControllerDevice, CONTROLLER_ID, register_input_natives};
#[cfg(feature = "compiler")]
use input::input_fn_decls;
use crate::plugin::Plugin;

pub struct GfxPlugin {
    pub fb: Rc<RefCell<Framebuffer>>,
    pub controller: Rc<RefCell<Controller>>,
    root: PathBuf,
}

impl GfxPlugin {
    pub fn new(headless: bool, root: PathBuf) -> Self {
        let mut fb = Framebuffer::new();
        if headless { fb.set_headless(); }
        Self {
            fb: Rc::new(RefCell::new(fb)),
            controller: Rc::new(RefCell::new(Controller::new())),
            root,
        }
    }
}

impl Plugin for GfxPlugin {
    fn install(&self, vm: &mut VirtualMachine) {
        vm.install_device(SCREEN_ID, Box::new(ScreenDevice::new(Rc::clone(&self.fb))));
        vm.install_device(CONTROLLER_ID, Box::new(ControllerDevice::new(Rc::clone(&self.controller), Rc::clone(&self.fb))));
        register_natives(vm, Rc::clone(&self.fb));
        register_input_natives(vm, Rc::clone(&self.controller), Rc::clone(&self.fb));
        #[cfg(feature = "fs")]
        register_io_natives(vm, Rc::clone(&self.fb), self.root.clone());
    }

    #[cfg(feature = "compiler")]
    fn register_fns(&self, compiler: &mut abrase::compiler::Compiler) -> Result<(), String> {
        use abrase::ast::EffectItem;
        let gfx_eff = || vec![EffectItem { name: vec!["Graphics".into()], arg: None }];
        let io_eff  = || vec![EffectItem { name: vec!["IO".into()], arg: None }];
        for (name, params, ret) in host_fn_decls() {
            compiler.register_host_fn(&format!("gfx_{name}"), params, ret, gfx_eff())?;
        }
        for (name, params, ret) in input_fn_decls() {
            compiler.register_host_fn(name, params, ret, io_eff())?;
        }
        #[cfg(feature = "fs")]
        for (name, params, ret) in host_fn_io_decls() {
            compiler.register_host_fn(&format!("gfx_{name}"), params, ret, io_eff())?;
        }
        Ok(())
    }
}

fn arg(args: &[Value], i: usize) -> i64 {
    args.get(i).copied().unwrap_or(Value::ZERO).as_int()
}

fn ret_unit() -> Result<(Value, bool), String> { Ok((Value::UNIT, false)) }

// Is the `bit`-th pixel of a 1bpp sprite set? MSB-first within each byte.
fn bit1(cells: &[u64], bit: usize) -> bool {
    let byte = cells.get(bit / 8).copied().unwrap_or(0) as u8;
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
        for dy in 0..h {
            for dx in 0..w {
                fbm.pset(x0 + dx, y0 + dy, c);
            }
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
        let (slot, gen_) = sprite.as_handle();
        let cells = ctx.heap.cell_data(slot, gen_)?;
        let (cx, cy) = (w as f64 / 2.0, h as f64 / 2.0);
        let (cxd, cyd) = (x0 + w / 2, y0 + h / 2);
        let theta = deg as f64 * std::f64::consts::PI / 180.0;
        let (s, c) = theta.sin_cos();
        let half = (((w * w + h * h) as f64).sqrt() / 2.0).ceil() as i64 + 1;
        let mut fbm = f.borrow_mut();
        let mut ddy = -half;
        while ddy <= half {
            let mut ddx = -half;
            while ddx <= half {
                // inverse rotate dest offset -> source pixel
                let sx = (c * ddx as f64 + s * ddy as f64 + cx).round() as i64;
                let sy = (-s * ddx as f64 + c * ddy as f64 + cy).round() as i64;
                if sx >= 0 && sx < w && sy >= 0 && sy < h {
                    let bit = off + (sy * w + sx) as usize;
                    if bit1(cells, bit) {
                        fbm.pset_op(cxd + ddx, cyd + ddy, color, mode);
                    }
                }
                ddx += 1;
            }
            ddy += 1;
        }
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
    let f = Rc::clone(&fb);
    reg!("gfx_blit", move |ctx: &mut NativeCtx, a: &[Value]| {
        let sprite = a.first().copied().unwrap_or(Value::NONE);
        let (x0, y0, w, h, color) = (arg(a, 1), arg(a, 2), arg(a, 3), arg(a, 4), arg(a, 5) as u16);
        if sprite.is_handle_none() { return ret_unit(); }
        let (slot, gen_) = sprite.as_handle();
        let cells = ctx.heap.cell_data(slot, gen_)?;
        let mut fbm = f.borrow_mut();
        for py in 0..h {
            for px in 0..w {
                let bit = (py * w + px) as usize;
                if bit1(cells, bit) {
                    fbm.pset(x0 + px, y0 + py, color);
                }
            }
        }
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
        let (slot, gen_) = sprite.as_handle();
        let cells = ctx.heap.cell_data(slot, gen_)?;
        let mut fbm = f.borrow_mut();
        for py in 0..h {
            for px in 0..w {
                let bit = off + (py * w + px) as usize;
                if bit1(cells, bit) {
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

#[cfg(feature = "fs")]
pub fn register_io_natives(vm: &mut VirtualMachine, fb: Rc<RefCell<Framebuffer>>, root: PathBuf) {
    let f = Rc::clone(&fb);
    let r = root.clone();
    let save_png = Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let (x, y, w, h) = (arg(a, 0), arg(a, 1), arg(a, 2), arg(a, 3));
        let Some(rel) = a.get(4).copied().and_then(|v| read_string(ctx.heap, v)) else {
            return Ok((Value::from_int(-1), false));
        };
        let Some(path) = crate::fs::resolve(&r, &rel) else {
            return Ok((Value::from_int(-1), false));
        };
        match f.borrow().save_region_png(x, y, w, h, &path) {
            Ok(()) => Ok((Value::from_int(0), false)),
            Err(e) => { eprintln!("save_png: {e}"); Ok((Value::from_int(-1), false)) }
        }
    });
    vm.register_native("gfx_save_png", save_png);
}

#[cfg(all(feature = "compiler", feature = "fs"))]
pub fn host_fn_io_decls() -> Vec<(&'static str, Vec<abrase::ty::Type>, abrase::ty::Type)> {
    use abrase::ty::Type as T;
    vec![
        ("save_png", vec![T::Int, T::Int, T::Int, T::Int, T::String], T::Int),
    ]
}

#[cfg(feature = "compiler")]
pub fn host_fn_decls() -> Vec<(&'static str, Vec<abrase::ty::Type>, abrase::ty::Type)> {
    use abrase::ty::Type as T;
    let arr_int = || T::Generic { name: "Array".into(), args: vec![T::Int] };
    let ref_bytes = || T::Reference { is_mut: false, inner: Box::new(T::Named("Bytes".into())) };
    vec![
        ("screen",      vec![T::Int, T::Int],                              T::Unit),
        ("screen_off",  vec![],                                            T::Unit),
        ("commit",      vec![],                                            T::Unit),
        ("cls",     vec![T::Int],                                          T::Unit),
        ("win_pos", vec![T::Int, T::Int],                                  T::Unit),
        ("pset",    vec![T::Int, T::Int, T::Int],                          T::Unit),
        ("rect",    vec![T::Int, T::Int, T::Int, T::Int, T::Int],          T::Unit),
        ("rectb",   vec![T::Int, T::Int, T::Int, T::Int, T::Int],          T::Unit),
        ("rectmix", vec![T::Int, T::Int, T::Int, T::Int, T::Int, T::Int],  T::Unit),
        ("dither",  vec![T::Int, T::Int],                                  T::Unit),
        ("line",    vec![T::Int, T::Int, T::Int, T::Int, T::Int],          T::Unit),
        ("linew",   vec![T::Int, T::Int, T::Int, T::Int, T::Int, T::Int],  T::Unit),
        ("circ",    vec![T::Int, T::Int, T::Int, T::Int],                  T::Unit),
        ("circb",   vec![T::Int, T::Int, T::Int, T::Int],                  T::Unit),
        ("tri",     vec![T::Int, T::Int, T::Int, T::Int, T::Int, T::Int, T::Int], T::Unit),
        ("trib",    vec![T::Int, T::Int, T::Int, T::Int, T::Int, T::Int, T::Int], T::Unit),
        ("pal",     vec![T::Int, T::Int],                                  T::Unit),
        ("blit",    vec![arr_int(), T::Int, T::Int, T::Int, T::Int, T::Int], T::Unit),
        ("blitg",   vec![arr_int(), T::Int, T::Int, T::Int, T::Int, T::Int, T::Int, T::Int], T::Unit),
        ("blitr",   vec![arr_int(), T::Int, T::Int, T::Int, T::Int, T::Int, T::Int, T::Int], T::Unit),
        ("sprite",  vec![ref_bytes(), T::Int, T::Int, T::Int, T::Int, T::Int, T::Int, T::Int], T::Unit),
    ]
}

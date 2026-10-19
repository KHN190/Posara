#[cfg(feature = "gfx-desktop")]
mod png;

pub use posara_core::gfx::*;

#[cfg(feature = "fs")]
use std::{cell::RefCell, path::PathBuf, rc::Rc};

#[cfg(any(feature = "fs", feature = "gfx-desktop"))]
use myriad::Value;
#[cfg(feature = "fs")]
use myriad::{NativeCtx, VirtualMachine};
#[cfg(all(feature = "fs", feature = "gfx-desktop"))]
use myriad::read_string;

#[cfg(feature = "gfx-desktop")]
pub use png::SavePng;

#[cfg(feature = "gfx-desktop")]
fn arg(args: &[Value], i: usize) -> i64 {
    args.get(i).copied().unwrap_or(Value::ZERO).as_int()
}

// gfx_save_png is a native on every backend so .pk carts resolve it. Desktop
// writes a PNG; web has no file sink (stub returns -1).
#[cfg(feature = "fs")]
pub fn register_io_natives(vm: &mut VirtualMachine, fb: Rc<RefCell<Framebuffer>>, root: PathBuf) {
    let _ = (&fb, &root);
    #[cfg(feature = "gfx-desktop")]
    {
        let (f, r) = (Rc::clone(&fb), root.clone());
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
    #[cfg(not(feature = "gfx-desktop"))]
    vm.register_native("gfx_save_png", Rc::new(|_: &mut NativeCtx, _: &[Value]| Ok((Value::from_int(-1), false))));
}


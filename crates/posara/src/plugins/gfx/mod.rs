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

// gfx_save_png is a native on every backend so .pk carts resolve it. Desktop
// writes a PNG; web has no file sink (stub returns -1).
#[cfg(feature = "fs")]
pub fn register_io_natives(vm: &mut VirtualMachine, fb: Rc<RefCell<Framebuffer>>, root: PathBuf) {
    let _ = (&fb, &root);
    #[cfg(feature = "gfx-desktop")]
    {
        let (f, r) = (Rc::clone(&fb), root.clone());
        let save_png = Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
            let a = posara_core::abi::Args(a);
            let (x, y, w, h) = (a.coord(0), a.coord(1), a.coord(2), a.coord(3));
            let Some(rel) = read_string(ctx.heap, a.handle(4)) else {
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


pub mod controller;
pub mod device;

pub use controller::{Button, Controller};
pub use device::{ControllerDevice, CONTROLLER_ID};

use std::cell::RefCell;
use std::rc::Rc;

use myriad::{NativeCtx, Value, VirtualMachine};

use crate::gfx::Framebuffer;

pub(crate) fn poll_controller(controller: &RefCell<Controller>, fb: &RefCell<Framebuffer>) {
    if let Some(win) = fb.borrow().window.as_ref() {
        controller.borrow_mut().poll(win);
    }
}

pub fn register_input_natives(
    vm: &mut VirtualMachine,
    controller: Rc<RefCell<Controller>>,
    fb: Rc<RefCell<Framebuffer>>,
) -> Vec<&'static str> {
    let (c, f) = (Rc::clone(&controller), Rc::clone(&fb));
    vm.register_native("in_buttons", Rc::new(move |_: &mut NativeCtx, _a: &[Value]| {
        poll_controller(&c, &f);
        Ok((Value::from_int(c.borrow().buttons as i64), false))
    }));
    let (c, f) = (controller, fb);
    vm.register_native("in_key", Rc::new(move |_: &mut NativeCtx, _a: &[Value]| {
        poll_controller(&c, &f);
        Ok((Value::from_int(c.borrow().key as i64), false))
    }));
    vec!["in_buttons", "in_key"]
}

#[cfg(feature = "compiler")]
pub fn input_fn_decls() -> Vec<(&'static str, Vec<abrase::ty::Type>, abrase::ty::Type)> {
    use abrase::ty::Type as T;
    vec![
        ("in_buttons", vec![], T::Int),
        ("in_key",     vec![], T::Int),
    ]
}

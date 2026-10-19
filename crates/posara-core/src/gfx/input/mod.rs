pub mod controller;
pub mod device;

pub use controller::{Button, Controller};
pub use device::{ControllerDevice, CONTROLLER_ID};

use core::cell::RefCell;
use alloc::rc::Rc;

use myriad::{NativeCtx, Value, VirtualMachine};

use crate::gfx::Framebuffer;
use crate::plugin::Plugin;
use crate::prelude::*;

// Controller device + in_buttons/in_key natives. Shares the gfx framebuffer's
// Rc so it can read window focus when polling, without owning the screen.
pub struct InputPlugin {
    pub controller: Rc<RefCell<Controller>>,
    fb: Rc<RefCell<Framebuffer>>,
}

impl InputPlugin {
    pub fn new(fb: Rc<RefCell<Framebuffer>>) -> Self {
        Self { controller: Rc::new(RefCell::new(Controller::new())), fb }
    }
}

impl Plugin for InputPlugin {
    fn install(&self, vm: &mut VirtualMachine) {
        vm.install_device(CONTROLLER_ID, Box::new(ControllerDevice::new(Rc::clone(&self.controller), Rc::clone(&self.fb))));
        register_input_natives(vm, Rc::clone(&self.controller), Rc::clone(&self.fb));
    }
}

pub fn poll_controller(controller: &RefCell<Controller>, fb: &RefCell<Framebuffer>) {
    if let Some((b, k)) = fb.borrow_mut().poll_input() {
        controller.borrow_mut().set(b, k);
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

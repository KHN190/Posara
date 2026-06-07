use std::cell::RefCell;
use std::rc::Rc;

use myriad::Device;
use myriad::memory::Heap;
use polka::Value;

use crate::gfx::Framebuffer;
use super::Controller;

pub const CONTROLLER_ID: u8 = 0x80;
pub const PORT_BUTTONS: u8 = 0x02;
pub const PORT_KEY:     u8 = 0x03;

pub struct ControllerDevice {
    controller: Rc<RefCell<Controller>>,
    fb: Rc<RefCell<Framebuffer>>,
}

impl ControllerDevice {
    pub fn new(controller: Rc<RefCell<Controller>>, fb: Rc<RefCell<Framebuffer>>) -> Self {
        Self { controller, fb }
    }

    fn refresh(&self) {
        let fb = self.fb.borrow();
        if let Some(win) = fb.window.as_ref() {
            self.controller.borrow_mut().poll(win);
        }
    }
}

impl Device for ControllerDevice {
    fn read(&mut self, port: u8) -> Result<(Value, bool), String> {
        self.refresh();
        match port {
            PORT_BUTTONS => Ok((Value::from_int(self.controller.borrow().buttons as i64), false)),
            PORT_KEY     => Ok((Value::from_int(self.controller.borrow().key as i64), false)),
            _ => Err(format!("Controller: read port {:#04x} unsupported", port)),
        }
    }

    fn write(&mut self, _port: u8, _val: Value, _is_handle: bool, _heap: &mut Heap) -> Result<(), String> {
        Err("Controller: device is read-only".into())
    }
}

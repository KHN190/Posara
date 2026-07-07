use std::cell::RefCell;
use std::rc::Rc;

use myriad::Device;
use myriad::memory::Heap;
use polka::Value;

use super::Framebuffer;

pub const SCREEN_ID: u8 = 0x20;
pub const PORT_CONFIGURE: u8 = 0x00;
pub const PORT_COMMIT:    u8 = 0x01;

pub struct ScreenDevice {
    fb: Rc<RefCell<Framebuffer>>,
}

impl ScreenDevice {
    pub fn new(fb: Rc<RefCell<Framebuffer>>) -> Self {
        Self { fb }
    }
}

impl Device for ScreenDevice {
    fn read(&mut self, port: u8) -> Result<(Value, bool), String> {
        match port {
            PORT_COMMIT => Ok((Value::from_int(if self.fb.borrow().alive { 1 } else { 0 }), false)),
            _ => Err(format!("Screen: read port {:#04x} unsupported", port)),
        }
    }

    fn write(&mut self, port: u8, val: Value, _is_handle: bool, _heap: &mut Heap) -> Result<(), String> {
        match port {
            PORT_CONFIGURE => {
                let packed = val.as_int() as u64;
                let w = (packed & 0xFFFF) as usize;
                let h = ((packed >> 16) & 0xFFFF) as usize;
                let fmt = ((packed >> 32) & 0xFF) as u8;
                self.fb.borrow_mut().configure(w, h, fmt)
            }
            PORT_COMMIT => self.fb.borrow_mut().commit(),
            _ => Err(format!("Screen: write port {:#04x} unsupported", port)),
        }
    }
}

use core::cell::RefCell;
use alloc::rc::Rc;

use super::runtime::Device;
use crate::prelude::*;
use super::runtime::{Heap, Value};

use super::Framebuffer;

pub const SCREEN_ID: u8 = crate::devices::SCREEN;
pub const PORT_CONFIGURE: u8 = 0x00;
pub const PORT_COMMIT:    u8 = 0x01;
pub const PORT_FORMAT:    u8 = 0x02;

pub struct ScreenDevice {
    fb: Rc<RefCell<Framebuffer>>,
    format: u8,
}

impl ScreenDevice {
    pub fn new(fb: Rc<RefCell<Framebuffer>>) -> Self {
        Self { fb, format: 1 }
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
                let packed = val.raw();
                let w = (packed & 0xFFFF) as usize;
                let h = ((packed >> 16) & 0xFFFF) as usize;
                let fmt = super::super::screen_configuration_format(packed, self.format);
                self.fb.borrow_mut().configure(w, h, fmt)
            }
            PORT_COMMIT => self.fb.borrow_mut().commit(),
            PORT_FORMAT if super::runtime::IS_INT32 => {
                let format = u8::try_from(val.as_int()).map_err(|_| String::from("Screen: invalid format"))?;
                if format != 1 { return Err(format!("Screen: format {} unsupported (only 1=RGB565)", format)); }
                self.format = format;
                Ok(())
            }
            _ => Err(format!("Screen: write port {:#04x} unsupported", port)),
        }
    }
}

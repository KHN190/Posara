use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use core::cell::RefCell;

use super::runtime::{Device, Heap, NativeCtx, Value, VirtualMachine, Int};

use crate::devices::MIDI;
use crate::drivers::MidiPort;
use super::plugin::Plugin;

pub const PORT_EVENT: u8 = 0x00;
pub const PORT_COUNT: u8 = 0x01;
pub const PORT_SEND: u8 = 0x02;

pub type SharedMidi = Rc<RefCell<Box<dyn MidiPort>>>;

pub fn message(w: u32) -> ([u8; 3], usize) {
    let [status, d1, d2, _] = w.to_le_bytes();
    let len = match status {
        0x80..=0xBF | 0xE0..=0xEF | 0xF2 => 3,
        0xC0..=0xDF | 0xF1 | 0xF3 => 2,
        0xF6 | 0xF8..=0xFF => 1,
        _ => 0,
    };
    ([status, d1 & 0x7F, d2 & 0x7F], len)
}

pub struct PortDevice {
    port: SharedMidi,
    queue: VecDeque<u32>,
}

impl PortDevice {
    pub fn new(port: SharedMidi) -> Self { Self { port, queue: VecDeque::new() } }

    fn pump(&mut self) {
        let mut p = self.port.borrow_mut();
        while let Some(ev) = p.recv() { self.queue.push_back(ev); }
    }
}

impl Device for PortDevice {
    fn read(&mut self, port: u8) -> Result<(Value, bool), String> {
        self.pump();
        match port {
            PORT_EVENT => Ok((Value::from_raw(self.queue.pop_front().unwrap_or(0) as super::Word), false)),
            PORT_COUNT => Ok((Value::from_int(Int::try_from(self.queue.len()).map_err(|_| String::from("MIDI event count exceeds Int"))?), false)),
            _ => Err(format!("MIDI: read port {:#04x} unsupported", port)),
        }
    }

    fn write(&mut self, port: u8, val: Value, _is_handle: bool, _heap: &mut Heap) -> Result<(), String> {
        match port {
            PORT_SEND => self.port.borrow_mut().send(val.as_int() as u32),
            _ => Err(format!("MIDI: write port {:#04x} unsupported", port)),
        }
    }
}

pub fn register_natives(vm: &mut VirtualMachine) {
    vm.register_native("midi_poll", Rc::new(|ctx: &mut NativeCtx, _: &[Value]| {
        match ctx.devices.get_mut(MIDI) {
            Some(d) => d.read(PORT_EVENT),
            None => Ok((Value::ZERO, false)),
        }
    }));
    vm.register_native("midi_count", Rc::new(|ctx: &mut NativeCtx, _: &[Value]| {
        match ctx.devices.get_mut(MIDI) {
            Some(d) => d.read(PORT_COUNT),
            None => Ok((Value::ZERO, false)),
        }
    }));
    vm.register_native("midi_send", Rc::new(|ctx: &mut NativeCtx, a: &[Value]| {
        let w = Value::from_int(super::abi::Args(a).word(0));
        if let Some(d) = ctx.devices.get_mut(MIDI) {
            d.write(PORT_SEND, w, false, ctx.heap)?;
        }
        Ok((Value::UNIT, false))
    }));
}

pub struct MidiPlugin {
    pub port: SharedMidi,
}

impl Plugin for MidiPlugin {
    fn install(&self, vm: &mut VirtualMachine) {
        vm.install_device(MIDI, Box::new(PortDevice::new(Rc::clone(&self.port))));
        register_natives(vm);
    }
}

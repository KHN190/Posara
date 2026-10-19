use alloc::boxed::Box;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use core::cell::RefCell;

use myriad::{Device, Heap, NativeCtx, Value, VirtualMachine};

use crate::devices::IMU;
use crate::drivers::Imu;
use crate::plugin::Plugin;

pub type SharedImu = Rc<RefCell<Box<dyn Imu>>>;

struct ImuDevice(SharedImu);

impl Device for ImuDevice {
    fn read(&mut self, port: u8) -> Result<(Value, bool), String> {
        match port {
            0..=2 => Ok((Value::from_int(self.0.borrow_mut().accel()[port as usize] as i64), false)),
            _ => Err(format!("IMU: read port {:#04x} unsupported", port)),
        }
    }

    fn write(&mut self, port: u8, _val: Value, _is_handle: bool, _heap: &mut Heap) -> Result<(), String> {
        Err(format!("IMU: write port {:#04x} unsupported", port))
    }
}

pub fn install(vm: &mut VirtualMachine, imu: SharedImu) {
    vm.install_device(IMU, Box::new(ImuDevice(imu)));
    vm.register_native("imu_accel", Rc::new(|ctx: &mut NativeCtx, a: &[Value]| {
        let axis = crate::abi::Args(a).word(0);
        match (u8::try_from(axis), ctx.devices.get_mut(IMU)) {
            (Ok(port @ 0..=2), Some(d)) => d.read(port),
            _ => Ok((Value::ZERO, false)),
        }
    }));
}

pub struct ImuPlugin {
    pub imu: SharedImu,
}

impl Plugin for ImuPlugin {
    fn install(&self, vm: &mut VirtualMachine) {
        install(vm, Rc::clone(&self.imu));
    }
}

use myriad::loader::LoadedModule;
use myriad::{Value, VirtualMachine};
use polka::Module;

use crate::prelude::*;

pub fn is_cart(module: &Module) -> bool {
    module.functions.iter().any(|c| matches!(c, polka::Chunk::Native(n) if n.name == "__frame_present"))
}

pub fn has_export(module: &Module, name: &str) -> bool {
    module.exports.iter().any(|e| e.name == name)
}

// Frame driver over a VM the platform already wired (devices + natives).
pub struct Stepper {
    vm: VirtualMachine,
    loaded: LoadedModule,
    cart: bool,
    has_update: bool,
    done: bool,
}

impl Stepper {
    pub fn start(vm: VirtualMachine, module: Module) -> Result<Self, String> {
        Self::start_with(vm, module, |_| {})
    }

    pub fn start_with<F: FnOnce(&mut VirtualMachine)>(mut vm: VirtualMachine, module: Module, prepare: F) -> Result<Self, String> {
        let loaded = myriad::loader::load(module)?;
        prepare(&mut vm);
        let cart = is_cart(&loaded.module);
        if cart {
            vm.run_to_yield(&loaded.module)?;
        } else if has_export(&loaded.module, "start") {
            vm.call_export(&loaded.module, "start", &[])?;
        }
        let has_update = has_export(&loaded.module, "update");
        Ok(Self { vm, loaded, cart, has_update, done: false })
    }

    pub fn is_frame_loop(&self) -> bool { self.cart || self.has_update }
    pub fn vm(&self) -> &VirtualMachine { &self.vm }
    pub fn steps(&self) -> u64 { self.vm.steps() }
    pub fn exit_code(&self) -> i64 { self.vm.exit_code().unwrap_or(0) }

    // One frame; false once the program is finished.
    pub fn frame(&mut self) -> Result<bool, String> {
        if self.done { return Ok(false); }
        if self.cart {
            if !self.vm.resume(&self.loaded.module, Value::from_int(0))? { self.done = true; }
        } else if self.has_update {
            self.vm.call_export(&self.loaded.module, "update", &[])?;
            if self.vm.halted() { self.done = true; }
        } else {
            let _: Value = self.vm.run_module(&self.loaded.module)?;
            self.done = true;
        }
        Ok(!self.done)
    }
}

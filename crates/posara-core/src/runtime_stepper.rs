use crate::{prelude::*, word32};
#[cfg(not(all(target_os = "none", target_pointer_width = "32")))]
use crate::word64;
use myriad::{Runtime, RuntimeProfile};
use polka::Module;

enum Backend {
    Word32(word32::stepper::Stepper),
    #[cfg(not(all(target_os = "none", target_pointer_width = "32")))]
    Word64(word64::stepper::Stepper),
}

pub struct RuntimeStepper { backend: Backend }

impl RuntimeStepper {
    pub fn start(module: Module) -> Result<Self, String> {
        Self::start_with(module, |_| {})
    }

    pub fn start_with(module: Module, prepare: impl FnOnce(&mut Runtime)) -> Result<Self, String> {
        let mut runtime = Runtime::new(&module)?;
        match runtime.profile() {
            RuntimeProfile::Word32 => myriad::word32::loader::validate(&module)?,
            #[cfg(not(all(target_os = "none", target_pointer_width = "32")))]
            RuntimeProfile::Word64 => myriad::word64::loader::validate(&module)?,
            #[cfg(all(target_os = "none", target_pointer_width = "32"))]
            RuntimeProfile::Word64 => return Err(String::from("INT32_SAFE cartridge required")),
        }
        prepare(&mut runtime);
        let backend = match runtime.profile() {
            RuntimeProfile::Word32 => Backend::Word32(word32::stepper::Stepper::start(
                runtime.into_word32().map_err(|_| String::from("runtime profile mismatch"))?, module)?),
            #[cfg(not(all(target_os = "none", target_pointer_width = "32")))]
            RuntimeProfile::Word64 => Backend::Word64(word64::stepper::Stepper::start(
                runtime.into_word64().map_err(|_| String::from("runtime profile mismatch"))?, module)?),
            #[cfg(all(target_os = "none", target_pointer_width = "32"))]
            RuntimeProfile::Word64 => return Err(String::from("INT32_SAFE cartridge required")),
        };
        Ok(Self { backend })
    }

    pub fn profile(&self) -> RuntimeProfile {
        match self.backend {
            Backend::Word32(_) => RuntimeProfile::Word32,
            #[cfg(not(all(target_os = "none", target_pointer_width = "32")))]
            Backend::Word64(_) => RuntimeProfile::Word64,
        }
    }

    pub fn frame(&mut self) -> Result<bool, String> {
        match &mut self.backend {
            Backend::Word32(stepper) => stepper.frame(),
            #[cfg(not(all(target_os = "none", target_pointer_width = "32")))]
            Backend::Word64(stepper) => stepper.frame(),
        }
    }

    pub fn is_frame_loop(&self) -> bool {
        match &self.backend {
            Backend::Word32(stepper) => stepper.is_frame_loop(),
            #[cfg(not(all(target_os = "none", target_pointer_width = "32")))]
            Backend::Word64(stepper) => stepper.is_frame_loop(),
        }
    }
}

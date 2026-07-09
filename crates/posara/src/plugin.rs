use myriad::VirtualMachine;

pub trait Plugin {
    fn install(&self, vm: &mut VirtualMachine);
    #[cfg(feature = "compiler")]
    fn register_fns(&self, compiler: &mut abrase::compiler::Compiler) -> Result<(), String>;
}

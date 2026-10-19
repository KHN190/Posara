use myriad::VirtualMachine;

pub trait Plugin {
    fn install(&self, vm: &mut VirtualMachine);
}

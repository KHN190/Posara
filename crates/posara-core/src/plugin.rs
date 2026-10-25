use super::runtime::VirtualMachine;

pub trait Plugin {
    fn install(&self, vm: &mut VirtualMachine);
}

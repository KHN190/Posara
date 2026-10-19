use std::collections::BTreeSet;

use myriad::devices::{SystemDevice, SYSTEM_ID};
use myriad::VirtualMachine;
use posara_core::abi::NATIVES;
use posara_core::host::{Drivers, Host};
use posara_core::stepper::Stepper;

fn fixture(name: &str) -> polka::Module {
    let p = format!("{}/../posara/tests/expected/{name}.pk", env!("CARGO_MANIFEST_DIR"));
    polka::cartridge::read_pk(&std::fs::read(p).unwrap()).unwrap()
}

fn vm_for(host: &Host) -> VirtualMachine {
    let mut vm = VirtualMachine::new();
    vm.install_device(SYSTEM_ID, Box::new(SystemDevice::new()));
    host.install(&mut vm);
    vm
}

#[test]
fn pk_from_full_desktop_build_loads_with_any_capabilities() {
    let host = Host::new(Drivers::null());
    let mut step = Stepper::start(vm_for(&host), fixture("full-native-table")).unwrap_or_else(|e| panic!("{e}"));
    assert!(step.frame().unwrap());
}

#[test]
fn native_names_are_unique() {
    let names: BTreeSet<_> = NATIVES.iter().map(|s| s.name).collect();
    assert_eq!(names.len(), NATIVES.len());
}

#[test]
fn time_follows_the_virtual_clock_when_set() {
    let host = Host::new(Drivers::null());
    host.clock.set(Some(1234));
    assert_eq!(host.now_ms(), 1234);
    host.clock.set(None);
    assert_eq!(host.now_ms(), 0);
}

#[cfg(not(all(feature = "fs", feature = "net")))]
#[test]
fn stubbed_text_and_bytes_natives_return_empty_values() {
    let host = Host::new(Drivers::null());
    let mut step = Stepper::start(vm_for(&host), fixture("stub-returns")).unwrap_or_else(|e| panic!("{e}"));
    step.frame().unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(step.exit_code(), 7);
}

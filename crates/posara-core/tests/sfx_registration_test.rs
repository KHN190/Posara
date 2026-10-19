#![cfg(feature = "sfx")]

use std::collections::BTreeSet;
use std::sync::Arc;

use myriad::VirtualMachine;
use posara_core::abi::{names, Cap};
use posara_core::sfx::{natives, spsc::Spsc};

#[test]
fn sfx_registered_names_match_native_table() {
    let mut vm = VirtualMachine::new();
    let registered: BTreeSet<_> = natives::register(&mut vm, Arc::new(Spsc::new(64))).into_iter().collect();
    assert_eq!(registered, names(Cap::Sfx).collect());
}

#[cfg(feature = "synth")]
#[test]
fn synth_registered_names_match_native_table() {
    let mut vm = VirtualMachine::new();
    let registered: BTreeSet<_> = natives::register_synth(&mut vm, Arc::new(Spsc::new(64))).into_iter().collect();
    assert_eq!(registered, names(Cap::Synth).collect());
}

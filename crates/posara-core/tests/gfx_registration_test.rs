#![cfg(feature = "gfx")]

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use myriad::VirtualMachine;
use posara_core::abi::{names, Cap};
use posara_core::gfx::{input, register_natives, Framebuffer};

#[test]
fn gfx_registered_names_match_native_table() {
    let mut vm = VirtualMachine::new();
    let fb = Rc::new(RefCell::new(Framebuffer::new()));
    let registered: BTreeSet<_> = register_natives(&mut vm, fb).into_iter().collect();
    assert_eq!(registered, names(Cap::Gfx).collect());
}

#[test]
fn input_registered_names_match_native_table() {
    let mut vm = VirtualMachine::new();
    let c = Rc::new(RefCell::new(input::Controller::new()));
    let fb = Rc::new(RefCell::new(Framebuffer::new()));
    let registered: BTreeSet<_> = input::register_input_natives(&mut vm, c, fb).into_iter().collect();
    assert_eq!(registered, names(Cap::Input).collect());
}

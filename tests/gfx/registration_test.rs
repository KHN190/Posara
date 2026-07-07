#![cfg(all(feature = "gfx", feature = "compiler"))]

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use posara::{Controller, Framebuffer, VirtualMachine};

#[test]
fn gfx_registered_names_match_decls() {
    let mut vm = VirtualMachine::new();
    let fb = Rc::new(RefCell::new(Framebuffer::new()));
    let registered: BTreeSet<String> = posara::gfx::register_natives(&mut vm, fb)
        .into_iter()
        .map(String::from)
        .collect();
    let declared: BTreeSet<String> = posara::gfx::host_fn_decls()
        .into_iter()
        .map(|(n, _, _)| format!("gfx_{n}"))
        .collect();
    assert_eq!(registered, declared, "gfx native registrations drifted from host_fn_decls");
}

#[test]
fn input_registered_names_match_decls() {
    let mut vm = VirtualMachine::new();
    let c = Rc::new(RefCell::new(Controller::new()));
    let fb = Rc::new(RefCell::new(Framebuffer::new()));
    let registered: BTreeSet<String> = posara::gfx::input::register_input_natives(&mut vm, c, fb)
        .into_iter()
        .map(String::from)
        .collect();
    let declared: BTreeSet<String> = posara::gfx::input::input_fn_decls()
        .into_iter()
        .map(|(n, _, _)| n.to_string())
        .collect();
    assert_eq!(registered, declared, "input native registrations drifted from decls");
}

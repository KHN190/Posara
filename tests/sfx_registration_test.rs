#![cfg(all(feature = "sfx", feature = "compiler"))]

use std::collections::BTreeSet;
use std::sync::Arc;

use posara::sfx::CmdProd;
use posara::VirtualMachine;

#[test]
fn sfx_registered_names_match_decls() {
    let prod: CmdProd = Arc::new(posara::sfx::spsc::Spsc::new(64));
    let mut vm = VirtualMachine::new();
    let registered: BTreeSet<String> = posara::sfx::register_natives(&mut vm, prod)
        .into_iter()
        .map(String::from)
        .collect();
    // record natives register separately (register_record_natives); exclude here.
    let declared: BTreeSet<String> = posara::sfx::host_fn_decls()
        .into_iter()
        .map(|(n, _, _)| n.to_string())
        .filter(|n| !n.starts_with("snd_bus_record"))
        .collect();
    assert_eq!(registered, declared, "sfx native registrations drifted from decls");
}

#[cfg(feature = "synth")]
#[test]
fn synth_registered_names_match_decls() {
    let prod: CmdProd = Arc::new(posara::sfx::spsc::Spsc::new(64));
    let mut vm = VirtualMachine::new();
    let registered: BTreeSet<String> =
        posara::plugins::synth::register_natives(&mut vm, prod)
            .into_iter()
            .map(String::from)
            .collect();
    let declared: BTreeSet<String> = posara::plugins::synth::host_fn_decls()
        .into_iter()
        .map(|(n, _, _)| n.to_string())
        .collect();
    assert_eq!(registered, declared, "synth native registrations drifted from decls");
}

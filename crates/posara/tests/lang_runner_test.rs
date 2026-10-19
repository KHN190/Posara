#![cfg(all(feature = "gfx", feature = "compiler"))]

use std::io::Write;

use posara::runner::{compile_abe, drive_realtime, drive_virtual, Stepper};
use posara::Host;

// drive_realtime must step the cart, invoke the hook each frame, and return once
// the cart finishes. A one-shot @cart presents once then main returns.
#[test]
fn drive_realtime_steps_and_hooks() {
    let mut p = std::env::temp_dir();
    p.push("posara_drive.abe");
    let src = "@cart\nfn main() -> <frame, Graphics, IO> Unit {\n  \
        gfx_screen(4, 4);\n  loop { gfx_cls(0x11); gfx_commit(); frame.present() }\n}\n";
    std::fs::File::create(&p).unwrap().write_all(src.as_bytes()).unwrap();

    let host = Host::new_with(std::path::PathBuf::from("."), true, true).unwrap();
    let r = compile_abe(&p, &host).unwrap();
    let mut step = Stepper::start_named(r.module, r.static_names, r.fn_names, &host).unwrap();

    let mut hooks = 0u64;
    drive_realtime(&mut step, &host, |_, _| { hooks += 1; Ok(hooks < 2) }).unwrap();

    assert!(hooks >= 1, "hook never ran");
    assert!(host.gfx.fb.borrow().buf.iter().all(|&c| c == 0x11), "cls did not run");
    std::fs::remove_file(&p).ok();
}

// A hook returning false stops the loop immediately.
#[test]
fn drive_realtime_hook_can_stop() {
    let mut p = std::env::temp_dir();
    p.push("posara_drive_stop.abe");
    // infinite loop cart; only the hook returning false ends it.
    let src = "@cart\nfn main() -> <frame, Graphics, IO> Unit {\n  \
        gfx_screen(4, 4);\n  loop { gfx_cls(0x22); gfx_commit(); frame.present() }\n}\n";
    std::fs::File::create(&p).unwrap().write_all(src.as_bytes()).unwrap();

    let host = Host::new_with(std::path::PathBuf::from("."), true, true).unwrap();
    let r = compile_abe(&p, &host).unwrap();
    let mut step = Stepper::start_named(r.module, r.static_names, r.fn_names, &host).unwrap();

    let mut hooks = 0u64;
    drive_realtime(&mut step, &host, |_, _| { hooks += 1; Ok(hooks < 3) }).unwrap();

    assert_eq!(hooks, 3, "hook stop not honored");
    std::fs::remove_file(&p).ok();
}

// drive_virtual advances a fake clock at 60Hz to the deadline; a loop cart runs
// exactly ceil-ish(deadline / (1000/60)) frames.
#[test]
fn drive_virtual_advances_to_deadline() {
    let mut p = std::env::temp_dir();
    p.push("posara_drive_virtual.abe");
    let src = "@cart\nfn main() -> <frame, Graphics, IO> Unit {\n  \
        gfx_screen(4, 4);\n  loop { gfx_cls(0x33); gfx_commit(); frame.present() }\n}\n";
    std::fs::File::create(&p).unwrap().write_all(src.as_bytes()).unwrap();

    let host = Host::new_with(std::path::PathBuf::from("."), true, true).unwrap();
    let r = compile_abe(&p, &host).unwrap();
    let mut step = Stepper::start_named(r.module, r.static_names, r.fn_names, &host).unwrap();

    let mut hooks = 0u64;
    drive_virtual(&mut step, &host, 50, |_, _| { hooks += 1; Ok(true) }).unwrap();

    // vms steps 0, 16.67, 33.33 are < 50; the next (50.0) stops the loop.
    assert_eq!(hooks, 3, "virtual clock frame count wrong");
    std::fs::remove_file(&p).ok();
}

fn run_one_frame(tag: &str, src: &str) -> i64 {
    let p = std::env::temp_dir().join(format!("posara_{tag}.abe"));
    std::fs::write(&p, src).unwrap();
    let host = Host::new_with(std::env::temp_dir(), true, true).unwrap();
    let r = compile_abe(&p, &host).unwrap_or_else(|e| panic!("{tag} compile:\n{e}"));
    let mut step = Stepper::start_named(r.module, r.static_names, r.fn_names, &host).unwrap();
    step.frame().unwrap();
    std::fs::remove_file(&p).ok();
    step.exit_code()
}

fn exit_of(code: i64) -> i64 {
    run_one_frame(&format!("halt_{}", code.unsigned_abs()),
        &format!("@cart\nfn main() -> <frame, IO> Unit {{\n  frame.present();\n  halt({code});\n  loop {{ frame.present() }}\n}}\n"))
}

#[test]
fn halt_code_is_a_32_bit_process_status() {
    assert_eq!(exit_of(0), 0);
    assert_eq!(exit_of(3) as i32, 3);
    assert_eq!(exit_of(-200) as i32, -200);
    assert_eq!(exit_of(0x1_0000_0007) as i32, 7);
}

#[test]
fn pk_needing_absent_native_fails_at_load_naming_it() {
    let p = std::env::temp_dir().join("posara_missing_native.abe");
    std::fs::write(&p, "@cart\nfn main() -> <frame, IO> Unit {\n  halt(in_buttons());\n  loop { frame.present() }\n}\n").unwrap();
    let host = Host::new_with(std::env::temp_dir(), true, true).unwrap();
    let r = compile_abe(&p, &host).unwrap_or_else(|e| panic!("compile:\n{e}"));
    std::fs::remove_file(&p).ok();
    let loaded = myriad::loader::load(r.module).unwrap();
    let err = posara::VirtualMachine::new().run_to_yield(&loaded.module).err().expect("bare VM must reject");
    assert!(err.starts_with("unresolved import: "), "{err}");
}

// expected/full-native-table.pk: packed by the desktop build with every capability; POSARA_UPDATE=1 re-packs.
fn full_native_table_pk() -> std::path::PathBuf {
    std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/expected/full-native-table.pk"))
}

// expected/stub-returns.pk: joins a text native with "abc", exits with lengths + 4.
#[cfg(all(feature = "desktop", feature = "test"))]
#[test]
fn repack_full_native_table_on_request() {
    if std::env::var("POSARA_UPDATE").as_deref() != Ok("1") { return; }
    let expected = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/expected"));
    let host = Host::new_with(std::env::temp_dir(), true, true).unwrap();
    for (name, body) in [
        ("full-native-table", "loop { frame.present() }"),
        ("stub-returns", "frame.present();\n  let s = fs_list(\"x\");\n  halt(\"{s}abc\".len() + udp_recv(0).len() + 4);\n  loop { frame.present() }"),
    ] {
        let p = std::env::temp_dir().join(format!("posara_{name}.abe"));
        std::fs::write(&p, format!("@cart\nfn main() -> <frame, IO, net> Unit {{\n  {body}\n}}\n")).unwrap();
        let r = compile_abe(&p, &host).unwrap_or_else(|e| panic!("{name}:\n{e}"));
        std::fs::write(expected.join(format!("{name}.pk")), posara::runner::write_pk_bytes(&r.module).unwrap()).unwrap();
    }
}

#[test]
fn pk_from_full_desktop_build_loads_in_every_build() {
    let bytes = std::fs::read(full_native_table_pk()).expect("full-native-table.pk missing; run desktop tests with POSARA_UPDATE=1");
    let host = Host::new_with(std::env::temp_dir(), true, true).unwrap();
    let mut step = Stepper::start(posara::runner::read_pk_bytes(&bytes).unwrap(), Vec::new(), &host)
        .unwrap_or_else(|e| panic!("{e}"));
    step.frame().unwrap();
}

fn run_int32(tag: &str, body: &str) -> i64 {
    let p = std::env::temp_dir().join(format!("posara_int32_{tag}.abe"));
    std::fs::write(&p, format!("@cart\nfn main() -> <frame, IO, nondet> Unit {{\n  frame.present();\n  {body}\n  loop {{ frame.present() }}\n}}\n")).unwrap();
    let host = Host::new_with(std::env::temp_dir(), true, true).unwrap();
    let r = posara::runner::compile_abe_with(&p, &host, true).unwrap_or_else(|e| panic!("{tag}:\n{e}"));
    std::fs::remove_file(&p).ok();
    assert_ne!(r.module.flags & polka::CART_FLAG_INT32_SAFE, 0);
    let mut step = Stepper::start(r.module, Vec::new(), &host).unwrap();
    step.frame().unwrap();
    step.exit_code()
}

#[test]
fn f32_cart_reads_rand_in_unit_range() {
    let body = "let mut ok = 0;\n  let mut i = 0;\n  while i < 64 {\n    let r = rand();\n    if r >= 0.0 && r < 1.0 { ok = ok + 1; }\n    i = i + 1;\n  }\n  halt(ok);";
    assert_eq!(run_int32("rand", body), 64);
}


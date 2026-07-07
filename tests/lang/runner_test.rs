#![cfg(all(feature = "gfx", feature = "compiler"))]

use std::io::Write;

use posara::runner::{compile_abe, drive_realtime, Stepper};
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

#![cfg(all(feature = "midi", feature = "compiler"))]

use std::io::Write;

use posara::runner::{compile_abe, Stepper};
use posara::Host;

// The midi_* natives route through the screen/midi device table; run a frame so
// a missing registration surfaces at dispatch, not just type-check.
#[test]
fn midi_prefixed_names_dispatch() {
    let mut p = std::env::temp_dir();
    p.push("posara_midi_names.abe");
    let src = "@cart\nfn main() -> <frame, IO> Unit {\n  \
        midi_send(144 + 60 * 256 + 100 * 65536);\n  \
        let _n = midi_count();\n  \
        let _e = midi_poll();\n  loop { frame.present() }\n}\n";
    std::fs::File::create(&p).unwrap().write_all(src.as_bytes()).unwrap();

    let host = Host::new_with(std::path::PathBuf::from("."), true, true).unwrap();
    let r = compile_abe(&p, &host).unwrap_or_else(|e| panic!("compile:\n{e}"));
    let mut step = Stepper::start_named(r.module, r.static_names, r.fn_names, &host).unwrap();
    step.frame().unwrap_or_else(|e| panic!("dispatch:\n{e}"));
    std::fs::remove_file(&p).ok();
}

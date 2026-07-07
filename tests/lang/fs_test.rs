#![cfg(all(feature = "fs", feature = "compiler"))]

use std::path::{Path, PathBuf};

use posara::fs::resolve;

#[test]
fn resolve_accepts_normal_and_curdir() {
    let root = Path::new("/root");
    assert_eq!(resolve(root, "a/b.spr"), Some(PathBuf::from("/root/a/b.spr")));
    assert_eq!(resolve(root, "./a"), Some(PathBuf::from("/root/a")));
}

#[test]
fn resolve_rejects_parent_escape() {
    let root = Path::new("/root");
    assert_eq!(resolve(root, ".."), None);
    assert_eq!(resolve(root, "../etc/passwd"), None);
    assert_eq!(resolve(root, "a/../../b"), None);
}

#[test]
fn resolve_rejects_absolute() {
    assert_eq!(resolve(Path::new("/root"), "/etc/passwd"), None);
}

// fs round-trip: a cart writes bytes then reads them back, proving
// fs_open/fs_writes/fs_close/fs_reads survive the native rename + wire together.
#[test]
fn fs_write_read_roundtrip() {
    use std::io::Write;
    use posara::Host;
    use posara::runner::{compile_abe, Stepper};

    let root = std::env::temp_dir().join("posara_fs_rt");
    std::fs::create_dir_all(&root).unwrap();
    let cart = root.join("rt.abe");
    // write "A" (0x41) to a file, read it back, paint it across the screen.
    let src = "@cart\nfn main() -> <frame, Graphics, IO> Unit {\n  \
        let w = fs_open(\"out.bin\", 22);\n  \
        let _ = fs_writes(w, \"A\");\n  \
        let _ = fs_close(w);\n  \
        gfx_screen(4, 4);\n  \
        loop {\n    \
        let r = fs_open(\"out.bin\", 1);\n    \
        let s = fs_reads(r, 1);\n    \
        let _ = fs_close(r);\n    \
        gfx_cls(s.byte_at(0));\n    \
        gfx_commit();\n    frame.present()\n  }\n}\n";
    std::fs::File::create(&cart).unwrap().write_all(src.as_bytes()).unwrap();

    let host = Host::new_with(root.clone(), true, true).unwrap();
    let r = compile_abe(&cart, &host).unwrap();
    let mut step = Stepper::start_named(r.module, r.static_names, r.fn_names, &host).unwrap();
    step.frame().unwrap();

    assert!(host.gfx.fb.borrow().buf.iter().all(|&c| c == 0x41), "round-trip byte mismatch");
    std::fs::remove_dir_all(&root).ok();
}

// fs_readb returns a Bytes value (8 bytes/word packed); byte_at must recover the
// original bytes, proving the packed representation round-trips.
#[test]
fn fs_readb_bytes_roundtrip() {
    use std::io::Write;
    use posara::Host;
    use posara::runner::{compile_abe, Stepper};

    let root = std::env::temp_dir().join("posara_fs_readb");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::File::create(root.join("blob.bin")).unwrap().write_all(&[0x11, 0x22, 0x33]).unwrap();
    let cart = root.join("rb.abe");
    // read 3 bytes as Bytes, paint the 3rd (0x33) across the screen.
    let src = "@cart\nfn main() -> <frame, Graphics, IO> Unit {\n  \
        let fd = fs_open(\"blob.bin\", 1);\n  \
        let b = fs_readb(fd, 3);\n  \
        let _ = fs_close(fd);\n  \
        gfx_screen(4, 4);\n  \
        loop { gfx_cls(b.byte_at(2)); gfx_commit(); frame.present() }\n}\n";
    std::fs::File::create(&cart).unwrap().write_all(src.as_bytes()).unwrap();

    let host = Host::new_with(root.clone(), true, true).unwrap();
    let r = compile_abe(&cart, &host).unwrap();
    let mut step = Stepper::start_named(r.module, r.static_names, r.fn_names, &host).unwrap();
    step.frame().unwrap();

    assert!(host.gfx.fb.borrow().buf.iter().all(|&c| c == 0x33), "fs_readb byte mismatch");
    std::fs::remove_dir_all(&root).ok();
}

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

// fs_read returns a packed Bytes value; byte_at must recover the original bytes.
// Reads a pre-existing on-disk file → real fs only (web MemStorage has no disk).
#[cfg(feature = "gfx-desktop")]
#[test]
fn fs_read_bytes_roundtrip() {
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
        let b = fs_read(fd, 3);\n  \
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

// End-to-end for the converged Bytes API: a `b"\xNN"` byte-string literal as a
// static 1bpp sprite, blit via &Bytes. Confirms the literal parses, statics of
// type Bytes work, and gfx_blitg renders from packed bytes at runtime.
#[test]
fn byte_literal_sprite_renders() {
    use std::io::Write;
    use posara::Host;
    use posara::runner::{compile_abe, Stepper};

    let root = std::env::temp_dir().join("posara_byte_lit");
    std::fs::create_dir_all(&root).unwrap();
    let cart = root.join("bl.abe");
    // b"\xff" = 8 set bits → an 8×1 solid 1bpp row.
    let src = "@cart\nfn main() -> <frame, Graphics, IO> Unit {\n  \
        let spr = b\"\\xff\";\n  \
        gfx_screen(8, 1);\n  \
        loop { gfx_cls(0x0000); gfx_blitg(&spr, 0, 0, 0, 8, 1, 0x1234, 0); gfx_commit(); frame.present() }\n}\n";
    std::fs::File::create(&cart).unwrap().write_all(src.as_bytes()).unwrap();

    let host = Host::new_with(root.clone(), true, true).unwrap();
    let r = compile_abe(&cart, &host).unwrap_or_else(|e| panic!("compile:\n{e}"));
    let mut step = Stepper::start_named(r.module, r.static_names, r.fn_names, &host).unwrap();
    step.frame().unwrap();

    assert!(host.gfx.fb.borrow().buf.iter().all(|&c| c == 0x1234), "byte-literal sprite did not render");
    std::fs::remove_dir_all(&root).ok();
}

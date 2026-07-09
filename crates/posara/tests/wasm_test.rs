#![cfg(all(feature = "gfx", feature = "compiler"))]

// Web/wasm seam: capability golden equivalence, the real headless=false entry,
// the pixel budget, and the MemStorage upload sandbox. All run natively under
// both `desktop` and `--no-default-features --features web,compiler`.

use posara::backend::storage::{MAX_FILES, MAX_FILE_BYTES};
use posara::backend::{MemStorage, Storage};
use posara::runner::{compile_abe, compile_source, compile_source_multi, read_pk_bytes, Stepper};
use posara::Host;

// Oracle: the web module resolver (compile_source_multi over MemStorage) must
// agree with the desktop loader (compile_abe) for the same multi-module cart.
// vis/acid.abe imports lib::proj / curl_noise / diff_growth / cube_slice.
#[test]
fn multi_module_resolver_matches_loader() {
    let base = concat!(env!("CARGO_MANIFEST_DIR"), "/../../carts");
    let host = Host::new_with(std::path::PathBuf::from("."), true, true).unwrap();

    let loader = compile_abe(&std::path::PathBuf::from(format!("{base}/vis/acid.abe")), &host);

    let store = MemStorage::new();
    store.add_file("acid.abe", std::fs::read(format!("{base}/vis/acid.abe")).unwrap()).unwrap();
    let lib = std::fs::read_dir(format!("{base}/lib")).unwrap();
    for e in lib.flatten() {
        let name = e.file_name().into_string().unwrap();
        if name.ends_with(".abe") {
            store.add_file(&format!("lib/{name}"), std::fs::read(e.path()).unwrap()).unwrap();
        }
    }
    let mine = compile_source_multi("acid.abe", &store, &host);

    assert_eq!(loader.is_ok(), mine.is_ok(),
        "resolver disagrees with loader.\nloader: {:?}\nmine: {:?}",
        loader.err(), mine.err());
}

const CART: &str = "@cart
fn main() -> <frame, Graphics, IO> Unit {
  gfx_screen(4, 4);
  loop { gfx_cls(0x1234); gfx_rect(1, 1, 2, 2, 0xABCD); gfx_commit(); frame.present() }
}
";

fn run_one_frame(src: &str) -> Host {
    let host = Host::new_with(std::path::PathBuf::from("."), true, true).unwrap();
    let r = compile_source(src, &host).unwrap();
    let mut step = Stepper::start_named(r.module, r.static_names, r.fn_names, &host).unwrap();
    step.frame().unwrap();
    host
}

// Same cart, same frame → identical framebuffer bytes on every backend.
#[test]
fn golden_frame_pixels_are_backend_independent() {
    let host = run_one_frame(CART);
    let fb = host.gfx.fb.borrow();
    assert_eq!((fb.w, fb.h), (4, 4));
    let expect: [u16; 16] = [
        0x1234, 0x1234, 0x1234, 0x1234,
        0x1234, 0xABCD, 0xABCD, 0x1234,
        0x1234, 0xABCD, 0xABCD, 0x1234,
        0x1234, 0x1234, 0x1234, 0x1234,
    ];
    assert_eq!(fb.buf, expect, "golden pixels differ — rendering changed");
    assert!(fb.commits >= 1, "commit never ran");
}

#[test]
fn code_mode_one_frame_no_panic() {
    let host = run_one_frame(CART);
    assert!(host.gfx.fb.borrow().commits >= 1);
}

// Real web entry path: headless=false (golden uses true). Excludes gfx-desktop
// (would open a window).
#[cfg(not(feature = "gfx-desktop"))]
#[test]
fn web_entry_headless_false_renders() {
    let host = Host::new_with(std::path::PathBuf::from("."), false, false).unwrap();
    let r = compile_source(CART, &host).unwrap();
    let mut step = Stepper::start_named(r.module, r.static_names, r.fn_names, &host).unwrap();
    step.frame().unwrap();
    let fb = host.gfx.fb.borrow();
    assert!(fb.commits >= 1 && fb.buf.iter().any(|&c| c != 0), "web entry produced no frame");
}

// 480 pixel budget rejects oversize screens on web backends only.
#[cfg(not(feature = "gfx-desktop"))]
#[test]
fn screen_over_budget_rejected() {
    let over = "@cart\nfn main() -> <frame, Graphics, IO> Unit {\n  \
        gfx_screen(600, 128);\n  loop { gfx_commit(); frame.present() }\n}\n";
    let host = Host::new_with(std::path::PathBuf::from("."), true, true).unwrap();
    let r = compile_source(over, &host).unwrap();
    let err = Stepper::start_named(r.module, r.static_names, r.fn_names, &host).is_err();
    assert!(err, "600px screen should exceed 480 budget");
}

// MemStorage = web upload sandbox: only added files are visible, quotas enforced
// in Rust, cart can't reach anything it wasn't given.
#[test]
fn sandbox_add_read_list_remove() {
    let s = MemStorage::new();
    s.add_file("a.abe", b"cart".to_vec()).unwrap();
    s.add_file("spr.spr", vec![1, 2, 3]).unwrap();
    assert_eq!(s.read_file("a.abe"), Some(b"cart".to_vec()));
    assert_eq!(s.file_names(), vec!["a.abe".to_string(), "spr.spr".to_string()]);
    assert!(s.remove("a.abe"));
    assert_eq!(s.read_file("a.abe"), None);
}

#[test]
fn sandbox_file_count_capped() {
    let s = MemStorage::new();
    for i in 0..MAX_FILES {
        s.add_file(&format!("f{i}.abe"), vec![0]).unwrap();
    }
    assert!(s.add_file("over.abe", vec![0]).is_err(), "17th file must reject");
    assert!(s.add_file("f0.abe", vec![9]).is_ok(), "overwrite at cap is fine");
}

#[test]
fn sandbox_per_file_size_capped() {
    let s = MemStorage::new();
    assert!(s.add_file("big", vec![0; MAX_FILE_BYTES + 1]).is_err());
    assert!(s.add_file("ok", vec![0; MAX_FILE_BYTES]).is_ok());
}

#[test]
fn sandbox_hides_unadded() {
    let s = MemStorage::new();
    assert_eq!(s.open("secret", 1), -1);
    assert!(!s.exists("secret"));
    assert_eq!(s.read_file("secret"), None);
}

#[test]
fn invader_pk_loads_and_runs() {
    let pk = concat!(env!("CARGO_MANIFEST_DIR"), "/../../carts/games/invader.pk");
    let bytes = std::fs::read(pk).expect("invader.pk missing — run: posara pack carts/games/invader.abe");
    let module = read_pk_bytes(&bytes).unwrap();

    let host = Host::new_with(std::path::PathBuf::from("."), true, true).unwrap();
    let mut step = Stepper::start_named(module, Vec::new(), Vec::new(), &host).unwrap();
    for _ in 0..3 { step.frame().unwrap(); }
    assert!(host.gfx.fb.borrow().commits >= 1, "pk cart never committed a frame");
}

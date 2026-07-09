#![cfg(all(feature = "compiler", feature = "synth"))]

use posara::Host;
use std::path::PathBuf;

// carts/ lives at the workspace root, two levels above this crate.
fn check(path: &str) {
    let full = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), path);
    let host = Host::new_with(PathBuf::from("."), true, true).expect("host init");
    let r = posara::runner::compile_abe(&PathBuf::from(&full), &host);
    assert!(r.is_ok(), "{path} failed to compile:\n{}", r.err().unwrap());
}

// compile AND run one-shot, so a missing/misnamed native surfaces at dispatch,
// not just at type-check.
fn run_src(tag: &str, src: &str) {
    use std::io::Write;
    use posara::runner::{compile_abe, Stepper};
    let mut p = std::env::temp_dir();
    p.push(format!("posara_{tag}.abe"));
    std::fs::File::create(&p).unwrap().write_all(src.as_bytes()).unwrap();
    let host = Host::new_with(PathBuf::from("."), true, true).expect("host init");
    let r = compile_abe(&p, &host).unwrap_or_else(|e| panic!("{tag} compile:\n{e}"));
    let mut step = Stepper::start_named(r.module, r.static_names, r.fn_names, &host).unwrap();
    step.frame().unwrap_or_else(|e| panic!("{tag} dispatch:\n{e}"));
    std::fs::remove_file(&p).ok();
}

#[test]
fn snd_prefixed_names_dispatch() {
    run_src("snd_names", "@cart\nfn main() -> <frame, IO> Unit {\n  snd_voices(8);\n  snd_osc(0, 0, 1, 0, 0, 80);\n  snd_on(0, 60, 80, 500);\n  snd_bus_reverb(20, 30, 40);\n  loop { frame.present() }\n}\n");
}

#[test]
fn hello_compiles() { check("carts/basic/hello.abe"); }

#[test]
fn text_compiles() { check("carts/basic/text.abe"); }

#[test]
fn sprite_compiles() { check("carts/basic/sprite.abe"); }

#[test]
fn invader_compiles() { check("carts/games/invader.abe"); }

#[test]
fn fuji_compiles() { check("carts/games/fuji.abe"); }

#[test]
fn song_compiles() { check("carts/music/song.abe"); }

#[test]
fn detroit_compiles() { check("carts/music/detroit.abe"); }

#[test]
fn acid_compiles() { check("carts/vis/acid.abe"); }

#[test]
fn kg_compiles() { check("carts/vis/kg.abe"); }

#[test]
fn micro_compiles() { check("carts/vis/micro.abe"); }

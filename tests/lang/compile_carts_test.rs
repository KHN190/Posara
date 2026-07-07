#![cfg(all(feature = "compiler", feature = "synth"))]

use posara::Host;
use std::path::PathBuf;

fn check(path: &str) {
    let host = Host::new_with(PathBuf::from("."), true, true).expect("host init");
    let r = posara::runner::compile_abe(&PathBuf::from(path), &host);
    assert!(r.is_ok(), "{path} failed to compile:\n{}", r.err().unwrap());
}

fn check_src(tag: &str, src: &str) {
    use std::io::Write;
    let mut p = std::env::temp_dir();
    p.push(format!("posara_{tag}.abe"));
    std::fs::File::create(&p).unwrap().write_all(src.as_bytes()).unwrap();
    let host = Host::new_with(PathBuf::from("."), true, true).expect("host init");
    let r = posara::runner::compile_abe(&p, &host);
    std::fs::remove_file(&p).ok();
    assert!(r.is_ok(), "{tag} failed to compile:\n{}", r.err().unwrap());
}

#[test]
fn snd_prefixed_names_compile() {
    check_src("snd_names", "@cart\nfn main() -> <IO> Unit {\n  snd_voices(8);\n  snd_osc(0, 0, 1, 0, 0, 80);\n  snd_on(0, 60, 80, 500);\n  snd_bus_reverb(20, 30, 40);\n  ()\n}\n");
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

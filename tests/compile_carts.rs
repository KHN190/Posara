#![cfg(all(feature = "compiler", feature = "synth"))]

use posara::Host;
use std::path::PathBuf;

fn check(path: &str) {
    let host = Host::new_with(PathBuf::from("."), true, true).expect("host init");
    let r = posara::runner::compile_abe(&PathBuf::from(path), &host);
    assert!(r.is_ok(), "{path} failed to compile:\n{}", r.err().unwrap());
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
fn ride_compiles() { check("carts/music/ride.abe"); }

#[test]
fn acid_compiles() { check("carts/vis/acid.abe"); }

#[test]
fn flute_compiles() { check("carts/vis/flute.abe"); }

#[test]
fn micro_compiles() { check("carts/vis/micro.abe"); }

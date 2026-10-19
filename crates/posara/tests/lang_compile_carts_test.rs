#![cfg(all(feature = "compiler", feature = "synth"))]

use posara::Host;
use std::path::PathBuf;

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


// POSARA_UPDATE=1 re-records tests/expected.
#[cfg(all(feature = "gfx", feature = "sfx"))]
mod recorded_output {
    #[cfg(feature = "sfx-desktop")]
    use posara::backend::Backend;
    use posara::runner::{compile_abe, default_root, read_pk_bytes, Stepper};
    use posara::Host;
    use std::path::{Path, PathBuf};

    const FRAMES: u64 = 300;
    const SAMPLES_PER_FRAME: usize = 44100 / 60 * 2;

    struct Fnv(u64);

    impl Fnv {
        fn new() -> Self { Fnv(0xcbf29ce484222325) }
        fn bytes(&mut self, b: &[u8]) {
            for &x in b { self.0 = (self.0 ^ x as u64).wrapping_mul(0x100000001b3); }
        }
    }

    fn carts_dir() -> PathBuf { PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../carts")) }

    fn entries() -> Vec<PathBuf> {
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    if p.file_name().is_some_and(|n| n != "lib") { walk(&p, out); }
                } else if p.extension().is_some_and(|x| x == "abe" || x == "pk") {
                    out.push(p);
                }
            }
        }
        let mut v = Vec::new();
        walk(&carts_dir(), &mut v);
        v.sort();
        v
    }

    fn run(path: &Path) -> Result<(u64, u64), String> {
        #[cfg(not(feature = "sfx-desktop"))]
        let mut host = Box::new(Host::new_with(default_root(path), true, true)?);
        #[cfg(not(feature = "sfx-desktop"))]
        upload_assets(&host, &default_root(path));
        #[cfg(feature = "sfx-desktop")]
        let host = Box::new(Host::with_backend(default_root(path), Backend::null(default_root(path)), true, true)?);
        // web-style alias: Stepper holds &Host, pull needs &mut.
        let h: &'static Host = unsafe { &*(&*host as *const Host) };
        host.clock.set(Some(0));
        let mut step = if path.extension().is_some_and(|x| x == "pk") {
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            Stepper::start(read_pk_bytes(&bytes)?, Vec::new(), h)?
        } else {
            let r = compile_abe(path, &host)?;
            Stepper::start_named(r.module, r.static_names, r.fn_names, h)?
        };
        let (mut gfx, mut sfx) = (Fnv::new(), Fnv::new());
        #[cfg_attr(feature = "sfx-desktop", allow(unused_mut))]
        let mut pcm = vec![0f32; SAMPLES_PER_FRAME];
        let frames = if step.is_frame_loop() { FRAMES } else { 1 };
        for i in 0..frames {
            host.clock.set(Some(i * 1000 / 60));
            let more = step.frame()?;
            #[cfg(not(feature = "sfx-desktop"))]
            host.sfx.audio.pull(&mut pcm);
            for s in &pcm { sfx.bytes(&s.to_bits().to_le_bytes()); }
            for px in &host.gfx.fb.borrow().buf { gfx.bytes(&px.to_le_bytes()); }
            if !more { break; }
        }
        Ok((gfx.0, sfx.0))
    }

    #[cfg(not(feature = "sfx-desktop"))]
    fn upload_assets(host: &Host, root: &Path) {
        fn walk(dir: &Path, root: &Path, host: &Host) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() { walk(&p, root, host); continue; }
                if p.extension().is_some_and(|x| x == "abe" || x == "pk") { continue; }
                let name = p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
                host.fs.storage.add_file(&name, std::fs::read(&p).unwrap()).unwrap();
            }
        }
        walk(root, root, host);
    }

    fn line(path: &Path) -> String {
        let name = path.strip_prefix(carts_dir()).unwrap().display().to_string();
        match run(path) {
            Ok((g, s)) => format!("{name} gfx={g:016x} sfx={s:016x}"),
            Err(e) => format!("{name} err {}", e.lines().next().unwrap_or("").replace(env!("CARGO_MANIFEST_DIR"), "")),
        }
    }

    #[test]
    fn pk_runs_identical_to_its_source() {
        for pk in entries().into_iter().filter(|p| p.extension().is_some_and(|x| x == "pk")) {
            let abe = pk.with_extension("abe");
            if abe.exists() { assert_eq!(run(&pk), run(&abe), "{}", pk.display()); }
        }
    }

    #[test]
    fn carts_render_and_sound_as_recorded() {
        #[cfg(not(feature = "sfx-desktop"))]
        let expected = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/expected/carts-web.txt"));
        #[cfg(feature = "sfx-desktop")]
        let expected = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/expected/carts-desktop.txt"));
        let got: Vec<String> = entries().iter().map(|p| line(p)).collect();
        if std::env::var("POSARA_UPDATE").as_deref() == Ok("1") {
            std::fs::create_dir_all(expected.parent().unwrap()).unwrap();
            std::fs::write(&expected, got.join("\n") + "\n").unwrap();
            return;
        }
        let want = std::fs::read_to_string(&expected).expect("expected output missing; run with POSARA_UPDATE=1");
        let want: Vec<&str> = want.lines().collect();
        let diff: Vec<String> = got.iter().zip(want.iter().copied().chain(std::iter::repeat("<missing>")))
            .filter(|(g, w)| g.as_str() != *w)
            .map(|(g, w)| format!("  want {w}\n  got  {g}"))
            .collect();
        assert!(diff.is_empty() && got.len() == want.len(), "cart output changed:\n{}", diff.join("\n"));
    }
}

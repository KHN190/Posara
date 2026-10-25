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


mod any_native_arguments {
    use std::sync::mpsc;
    use std::time::Duration;

    use myriad::devices::{SystemDevice, SYSTEM_ID};
    use myriad::VirtualMachine;
    use posara_core::abi::{stub_all, Ty, NATIVES};
    use posara_core::stepper::Stepper;
    use proptest::prelude::*;

    const WATCHDOG: Duration = Duration::from_secs(10);

    fn int() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("(0 - 9223372036854775807 - 1)".to_string()), Just("9223372036854775807".into()), Just("0".into()),
            Just("(0 - 1)".into()), Just("1099511627776".into()), (-300i64..300).prop_map(|n| format!("({n})")), any::<i64>().prop_map(|n| format!("({n})")),
        ]
    }

    fn text() -> impl Strategy<Value = String> {
        prop_oneof![Just(String::new()), "[a-z./]{0,12}", Just("../../x".into()), Just("é".repeat(20000))]
            .prop_map(|s| format!("{s:?}"))
    }

    fn bytes() -> impl Strategy<Value = String> {
        prop_oneof![proptest::collection::vec(any::<u8>(), 0..48), Just(vec![0xFF; 1 << 15])]
            .prop_map(|b| format!("b\"{}\"", b.iter().map(|x| format!("\\x{x:02x}")).collect::<String>()))
    }

    fn ints() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("[0; 0]".to_string()), Just("[9223372036854775807; 4096]".into()),
            proptest::collection::vec(prop_oneof![any::<i64>(), -2i64..300], 1..40)
                .prop_map(|v| format!("[{}]", v.iter().map(|n| format!("({n})")).collect::<Vec<_>>().join(", "))),
        ]
    }

    fn arg(t: Ty) -> BoxedStrategy<String> {
        match t {
            Ty::Int | Ty::Float => int().boxed(),
            Ty::Bool => prop_oneof![Just("true".to_string()), Just("false".into())].boxed(),
            Ty::Unit => Just("()".to_string()).boxed(),
            Ty::Str => text().boxed(),
            Ty::Bytes | Ty::RefBytes => bytes().boxed(),
            Ty::ArrInt | Ty::RefArrInt => ints().boxed(),
        }
    }

    pub fn cart() -> impl Strategy<Value = (usize, String)> {
        (0..NATIVES.len()).prop_flat_map(|i| {
            let s = &NATIVES[i];
            (Just(i), s.params.iter().map(|&t| arg(t)).collect::<Vec<_>>())
        }).prop_map(|(i, args)| {
            let s = &NATIVES[i];
            let mut lets = String::new();
            let call: Vec<String> = s.params.iter().zip(&args).enumerate().map(|(k, (t, a))| match t {
                Ty::RefBytes | Ty::RefArrInt => { lets += &format!("  let a{k} = {a};\n"); format!("&a{k}") }
                Ty::Bytes | Ty::ArrInt | Ty::Str => { lets += &format!("    let a{k} = {a};\n"); format!("a{k}") }
                _ => a.clone(),
            }).collect();
            let (outer, inner): (Vec<&str>, Vec<&str>) = lets.lines().partition(|l| !l.starts_with("    "));
            let invocation=format!("{}({})",s.name,call.join(", "));
            let invoke=if s.eff==posara_core::abi::Eff::IOExnString {
                format!("match {invocation} {{ Ok(_) => (), Err(_) => (), _ => () }}")
            } else {invocation};
            (i, format!("@cart\nfn main() -> <frame, Graphics, IO, nondet, net> Unit {{\n{}\n  loop {{\n{}\n    let _ = {};\n    frame.present()\n  }}\n}}\n",
                outer.join("\n"), inner.join("\n"), invoke))
        })
    }

    fn live_after_frames(mut step: Stepper) -> Result<(), String> {
        let counts: Vec<usize> = (0..3).map(|_| { let _ = step.frame(); step.vm().heap_live_count() }).collect();
        if counts[2] > counts[0] { return Err(format!("live cells grow per call: {counts:?}")); }
        Ok(())
    }

    fn boot(module: polka::Module, install: impl FnOnce(&mut VirtualMachine)) -> Result<(), String> {
        let mut vm = VirtualMachine::new();
        vm.install_device(SYSTEM_ID, Box::new(SystemDevice::new()));
        install(&mut vm);
        match Stepper::start(vm, module) {
            Ok(step) => live_after_frames(step),
            Err(_) => Ok(()),
        }
    }

    fn guarded(name: &'static str, src: String) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let root = std::env::temp_dir().join(format!("posara_fuzz_{}_{name}", std::process::id()));
            std::fs::create_dir_all(&root).unwrap();
            let host = posara::Host::with_backend(root.clone(), posara::backend::Backend::null(root.clone()), true, true).unwrap();
            let module = || posara::runner::compile_source(&src, &host).map(|r| r.module).map_err(|e| format!("compile:\n{e}\n{src}"));
            let r = (|| {
                let m = module()?;
                host.load(&m);
                boot(m, |vm| host.install(vm)).map_err(|e| format!("desktop/web host: {e}"))?;
                let core = posara_core::host::Host::new(posara_core::host::Drivers::null());
                boot(module()?, |vm| core.install(vm)).map_err(|e| format!("core host: {e}"))?;
                boot(module()?, stub_all).map_err(|e| format!("stubs: {e}"))
            })();
            std::fs::remove_dir_all(&root).ok();
            let _ = tx.send(r);
        });
        match rx.recv_timeout(WATCHDOG) {
            Ok(r) => r,
            Err(mpsc::RecvTimeoutError::Timeout) => Err(format!("{name} hung over {WATCHDOG:?}")),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(format!("{name} panicked")),
        }
    }

    fn frame_with(body: &str) -> Result<(), String> {
        guarded("regression", format!("@cart\nfn main() -> <frame, Graphics, IO> Unit {{\n  {body};\n  loop {{ frame.present() }}\n}}\n"))
    }

    #[test]
    fn a_screen_too_big_for_memory_is_refused() {
        frame_with("gfx_screen(1099511627776, 1099511627776)").unwrap();
    }

    #[test]
    fn extreme_drawing_arguments_neither_hang_nor_overflow() {
        frame_with("gfx_screen(8, 8);\n  let b = b\"\\x00\";\n  gfx_blitg(&b, 0, 0, 0, 0 - 1, 9223372036854775807, 1, 0);\n  \
            gfx_line(0 - 9223372036854775807 - 1, 0, 3, 3, 1);\n  gfx_linew(0, 0, 7, 7, 9223372036854775807, 1);\n  \
            gfx_circb(0 - 9223372036854775807 - 1, 0, 3, 1);\n  gfx_rect(0 - 9223372036854775807, 0, 9223372036854775807, 9223372036854775807, 1)").unwrap();
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: ProptestConfig::default().cases.min(16), ..ProptestConfig::default() })]

        #[test]
        fn every_native_survives_any_arguments((i, src) in cart()) {
            let r = guarded(NATIVES[i].name, src.clone());
            prop_assert!(r.is_ok(), "{}\n{}", r.unwrap_err(), src);
        }
    }
}


#[cfg(feature = "sfx")]
mod sequence_contract {
use posara::{Host, runner::{compile_abe_with, read_pk_bytes, write_pk_bytes, Stepper}};
use posara_core::sfx::{mixer::Cmd, seq::{decode, pack}};

fn run(tag: &str, body: &str, int32: bool) -> Option<Cmd> {
    let path = std::env::temp_dir().join(format!("posara_sequence_{tag}_{}.abe", std::process::id()));
    std::fs::write(&path, format!("@cart\nfn main() -> <frame, IO> Unit {{\n frame.present();\n {body};\n loop {{ frame.present() }}\n}}\n")).unwrap();
    let host = Host::new_with(std::env::temp_dir(), true, true).unwrap();
    let result = compile_abe_with(&path, &host, int32);
    std::fs::remove_file(&path).unwrap();
    let result = result.unwrap_or_else(|error| panic!("{error}"));
    let module = read_pk_bytes(&write_pk_bytes(&result.module).unwrap()).unwrap();
    let mut step = Stepper::start(module, Vec::new(), &host).unwrap();
    step.frame().unwrap();
    host.sfx.audio.cmds.try_pop()
}

#[test]
fn byte_sequences_preserve_every_field_in_both_integer_modes() {
    let word=pack(65535,3,127,100,65535);
    let bytes:String=word.to_le_bytes().iter().map(|b|format!("\\x{b:02x}")).collect();
    for int32 in [false,true] {
        let Some(Cmd::Seq(events,ms))=run(if int32{"i32"}else{"i64"},&format!("snd_seq(b\"{bytes}\",120)"),int32) else{panic!("expected sequence")};
        assert_eq!(ms,120);assert_eq!(events.len(),1);
        let e=events[0];assert_eq!((e.tick,e.ch,e.note,e.dur),(65535,3,127,65535));assert_eq!(e.vel,1.0);
    }
}

#[test]
fn legacy_track_layout_keeps_padding_partial_words_and_stable_tick_order() {
    let mut bytes=Vec::new();
    for (tick,note) in [(4,60),(0,61),(4,62)]{bytes.extend(pack(tick,0,note,80,1).to_le_bytes());}
    bytes.extend([0;8]);bytes.push(1);
    let events=decode(&bytes);
    assert_eq!(events.iter().map(|e|(e.tick,e.note)).collect::<Vec<_>>(),vec![(0,61),(1,0),(4,60),(4,62)]);
}

#[test]
fn empty_sequences_stop_without_an_integer_width_restriction() {
    for int32 in [false,true]{
        assert!(matches!(run(if int32{"empty32"}else{"empty64"},"snd_seq(b\"\",120)",int32),Some(Cmd::Seq(events,120)) if events.is_empty()));
    }
}

#[test]
fn sequence_data_is_bytes_and_the_redundant_native_is_not_registered() {
    let host=Host::new_with(std::env::temp_dir(),true,true).unwrap();
    for call in ["snd_seq([1],120)","snd_track(b\"\",120)"] {
        assert!(posara::runner::compile_source(&format!("@cart\nfn main()-><frame,IO> Unit {{{call};loop{{frame.present()}}}}"),&host).is_err());
    }
}
}

#[test]
fn dual_width_carts_invalid_carts_and_reload_after_failure() {
    let host = Host::new_with(std::path::PathBuf::from("."), true, true).unwrap();

    let path32 = std::env::temp_dir().join(format!("posara_dw32_{}.abe", std::process::id()));
    let src32 = "@cart\nfn main() -> <frame, Graphics, IO> Unit {\n  gfx_screen(8, 8);\n  let mut x = 0;\n  while x < 4 {\n    gfx_rect(x, x, 2, 2, 0x1234);\n    x = x + 1;\n  }\n  loop { gfx_commit(); frame.present() }\n}\n";
    std::fs::write(&path32, src32).unwrap();
    let r32 = posara::runner::compile_abe_with(&path32, &host, true).unwrap();
    std::fs::remove_file(&path32).ok();
    assert_ne!(r32.module.flags & polka::CART_FLAG_INT32_SAFE, 0);
    let bytes32 = posara::runner::write_pk_bytes(&r32.module).unwrap();

    let src64 = "@cart\nfn main() -> <frame, Graphics, IO> Unit {\n  gfx_screen(16, 16);\n  gfx_rect(2, 2, 4, 4, 0x5678);\n  loop { gfx_commit(); frame.present() }\n}\n";
    let r64 = posara::runner::compile_source(src64, &host).unwrap();
    assert_eq!(r64.module.flags & polka::CART_FLAG_INT32_SAFE, 0);
    let bytes64 = posara::runner::write_pk_bytes(&r64.module).unwrap();

    let corrupt_bytes = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02];
    assert!(posara::runner::read_pk_bytes(&corrupt_bytes).is_err());
    assert!(posara::runner::read_pk_bytes(&[]).is_err());
    assert!(posara::runner::read_pk_bytes(&bytes32[..bytes32.len().min(10)]).is_err());

    let mod32 = posara::runner::read_pk_bytes(&bytes32).unwrap();
    assert_ne!(mod32.flags & polka::CART_FLAG_INT32_SAFE, 0);
    {
        host.gfx.fb.borrow_mut().reset();
        let mut step = Stepper::start_named(mod32, Vec::new(), Vec::new(), &host).unwrap();
        assert!(step.frame().unwrap());
        assert_eq!((host.gfx.fb.borrow().w, host.gfx.fb.borrow().h), (8, 8));
        assert!(host.gfx.fb.borrow().commits >= 1);
    }

    assert!(posara::runner::read_pk_bytes(&corrupt_bytes).is_err());

    let mod64 = posara::runner::read_pk_bytes(&bytes64).unwrap();
    assert_eq!(mod64.flags & polka::CART_FLAG_INT32_SAFE, 0);
    {
        host.gfx.fb.borrow_mut().reset();
        let mut step = Stepper::start_named(mod64, Vec::new(), Vec::new(), &host).unwrap();
        assert!(step.frame().unwrap());
        assert_eq!((host.gfx.fb.borrow().w, host.gfx.fb.borrow().h), (16, 16));
        assert!(host.gfx.fb.borrow().commits >= 1);
    }

    assert!(posara::runner::read_pk_bytes(&[0xFF; 20]).is_err());

    let mod32_again = posara::runner::read_pk_bytes(&bytes32).unwrap();
    {
        host.gfx.fb.borrow_mut().reset();
        let mut step = Stepper::start_named(mod32_again, Vec::new(), Vec::new(), &host).unwrap();
        assert!(step.frame().unwrap());
        assert_eq!((host.gfx.fb.borrow().w, host.gfx.fb.borrow().h), (8, 8));
        assert!(host.gfx.fb.borrow().commits >= 1);
    }
}

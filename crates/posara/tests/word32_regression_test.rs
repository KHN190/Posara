#![cfg(all(
    feature = "compiler",
    feature = "midi",
    feature = "imu",
    feature = "net"
))]

use std::cell::RefCell;
use std::io::{BufRead, BufReader};
use std::net::UdpSocket;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::rc::Rc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use posara::Host;
use posara::backend::{Backend, QueueMidi};
use posara::runner::{DebugCfg, Stepper, compile_abe_with, run_module};

struct Scratch(PathBuf);
impl Scratch {
    fn new(tag: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("posara_regression_{}_{tag}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct FixedImu;
impl posara_core::drivers::Imu for FixedImu {
    fn accel(&mut self) -> [i32; 3] {
        [100, -200, 500]
    }
}

#[test]
fn injected_midi_and_imu_survive_both_word_sizes() {
    for int32 in [false, true] {
        let scratch = Scratch::new(if int32 { "ports32" } else { "ports64" });
        let port = QueueMidi::default();
        let mut backend = Backend::null(scratch.0.clone()).imu(Box::new(FixedImu));
        backend.midi = Some(Rc::new(RefCell::new(Box::new(port.clone()))));
        let host = Host::with_backend(scratch.0.clone(), backend, true, true).unwrap();
        let path = scratch.0.join("ports.abe");
        std::fs::write(&path, "@cart\nfn main() -> <frame, IO> Unit {\n frame.present();\n if imu_accel(0) != 100 { halt(1) };\n if imu_accel(1) != -200 { halt(2) };\n if imu_accel(2) != 500 { halt(3) };\n if midi_count() != 1 { halt(4) };\n midi_send(midi_poll());\n halt(73);\n loop { frame.present() }\n}\n").unwrap();
        let r = compile_abe_with(&path, &host, int32).unwrap();
        let mut step = Stepper::start_named(r.module, r.static_names, r.fn_names, &host).unwrap();
        // The top byte exercises MIDI routing tags, including a signed i32 word.
        port.push(0xAB643C90);
        assert!(!step.frame().unwrap());
        assert_eq!(step.exit_code(), 73, "int32={int32}");
        assert_eq!(port.take_bytes(), [0x90, 60, 100], "int32={int32}");
    }
}

#[test]
fn udp_roundtrip_and_socket_cleanup_survive_both_word_sizes() {
    for int32 in [false, true] {
        let scratch = Scratch::new(if int32 { "udp32" } else { "udp64" });
        let peer = UdpSocket::bind("127.0.0.1:0").unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let addr = peer.local_addr().unwrap();
        let host = Host::with_backend(
            scratch.0.clone(),
            Backend::null(scratch.0.clone()),
            true,
            true,
        )
        .unwrap();
        let path = scratch.0.join("udp.abe");
        std::fs::write(&path, format!("@cart\nfn main() -> <frame, IO, net> Unit {{\n let s = udp_bind(0);\n let msg = [0, 127, 128, 255];\n let _ = udp_send(s, \"{addr}\", &msg);\n frame.present();\n loop {{\n let reply = udp_recv(s);\n if reply.len() > 0 {{\n let pong = [reply.byte_at(0), reply.byte_at(1)];\n let _ = udp_send(s, udp_from(s), &pong);\n halt(73)\n }};\n frame.present()\n }}\n}}\n")).unwrap();
        let r = compile_abe_with(&path, &host, int32).unwrap();
        let mut step = Stepper::start_named(r.module, r.static_names, r.fn_names, &host).unwrap();
        let mut buf = [0; 32];
        let (n, from) = peer.recv_from(&mut buf).unwrap();
        assert_eq!(&buf[..n], [0, 127, 128, 255]);
        assert_eq!(
            host.net.net.borrow().live(),
            1,
            "socket must belong to the original host"
        );
        peer.send_to(&[255, 128], from).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while step.frame().unwrap() {
            assert!(Instant::now() < deadline, "no UDP reply in int32={int32}");
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(step.exit_code(), 73);
        let (n, _) = peer.recv_from(&mut buf).unwrap();
        assert_eq!(&buf[..n], [255, 128]);
        drop(step);
        let r = compile_abe_with(&path, &host, int32).unwrap();
        let _next = Stepper::start_named(r.module, r.static_names, r.fn_names, &host).unwrap();
        assert_eq!(
            host.net.net.borrow().live(),
            1,
            "old sockets must be swept on reload"
        );
        // The old port is available again, even while the new cart is running.
        let _old_port = UdpSocket::bind(from).unwrap();
    }
}

struct Running(Child);
impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn reload_child() {
    let Some(path) = std::env::var_os("POSARA_REGRESSION_CART") else {
        return;
    };
    let path = PathBuf::from(path);
    let root = path.parent().unwrap().to_path_buf();
    let host = Host::with_backend(root.clone(), Backend::null(root), true, true).unwrap();
    let int32 = std::env::var("POSARA_REGRESSION_INT32").unwrap() == "1";
    let r = compile_abe_with(&path, &host, int32).unwrap();
    let code = run_module(
        r.module,
        r.static_names,
        r.fn_names,
        &host,
        Some(path),
        false,
        DebugCfg {
            trace: true,
            handlers: true,
            leak: true,
        },
    )
    .unwrap();
    assert_eq!(code, if int32 { 73 } else { 74 });
}

#[test]
fn real_reload_keeps_running_after_compile_failure_and_preserves_word_size() {
    for int32 in [false, true] {
        let scratch = Scratch::new(if int32 { "reload32" } else { "reload64" });
        let path = scratch.0.join("reload.abe");
        std::fs::write(
            &path,
            "@cart\nfn main() -> <frame, IO> Unit { loop { frame.present() } }\n",
        )
        .unwrap();
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "reload_child", "--nocapture"])
            .env("POSARA_REGRESSION_CART", &path)
            .env("POSARA_REGRESSION_INT32", if int32 { "1" } else { "0" })
            .env("PROFILE", "1")
            .env("BREAK_AT", "main:0")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut child = Running(child);
        let stderr = child.0.stderr.take().unwrap();
        let (tx, rx) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            let mut log = String::new();
            for line in BufReader::new(stderr).lines() {
                let line = line.unwrap();
                let _ = tx.send(line.clone());
                log.push_str(&line);
                log.push('\n');
            }
            log
        });
        let wait_for = |needle: &str| {
            let deadline = Instant::now() + Duration::from_secs(8);
            loop {
                let line = rx
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .unwrap_or_else(|e| panic!("waiting for {needle}, int32={int32}: {e}"));
                if line.contains(needle) {
                    break;
                }
            }
        };
        wait_for("hot reload armed:");
        std::fs::write(&path, "invalid source for reload\n").unwrap();
        wait_for("reload failed (keeping old):");
        assert!(child.0.try_wait().unwrap().is_none());
        std::fs::write(&path, "effect Probe { op ask() -> Int }\nfn probe() -> <Probe> Int { Probe.ask() }\n@cart\nfn main() -> <frame, IO> Unit {\n frame.present();\n let code = handle probe() { return v => v, Probe.ask => { let value = resume(1); value + 1 } };\n if code != 2 { halt(5) };\n let x = 16777216.0;\n if x + 1.0 == x { halt(73) } else { halt(74) };\n loop { frame.present() }\n}\n").unwrap();
        wait_for("• reloaded ");
        let deadline = Instant::now() + Duration::from_secs(8);
        let status = loop {
            if let Some(status) = child.0.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline, "reloaded cart did not exit");
            std::thread::sleep(Duration::from_millis(10));
        };
        let log = reader.join().unwrap();
        assert!(status.success(), "int32={int32}: {log}");
        assert!(log.contains("[break "), "missing breakpoint: {log}");
        // Myriad currently emits no HandlePush event; verify the actual Handle
        // opcode and the non-tail resume event separately.
        assert!(log.contains("Handle("), "missing handler execution: {log}");
        assert!(log.contains("Raise("), "missing effect execution: {log}");
        assert!(log.contains("[resume]"), "missing resume event: {log}");
        assert!(log.contains("(base r"), "missing register dump: {log}");
        assert!(log.contains("[heap]"), "missing leak report: {log}");
        assert!(log.contains("[profile]"), "missing profile report: {log}");
        assert!(log.contains("PushConst("), "missing opcode trace: {log}");
    }
}

#[cfg(all(feature = "sfx", not(feature = "sfx-desktop")))]
#[test]
fn shared_audio_preserves_pcm_state_across_pulls_and_owner_drop() {
    use posara::sfx::{Audio, mixer::Cmd};
    for sr in [44_100, 48_000] {
        let mut baseline = Audio::web(sr).unwrap();
        let mut original = Audio::web(sr).unwrap();
        for audio in [&baseline, &original] {
            assert!(audio.cmds.try_push(Cmd::Tone(437.0, 1000, 0.8, 0)).is_ok());
        }
        let mut expected = vec![0.0; 256];
        let mut actual = vec![0.0; 256];
        baseline.pull(&mut expected);
        original.pull(&mut actual);
        assert_eq!(actual, expected);
        let mut shared = original.share();
        for block in 0..8 {
            baseline.pull(&mut expected);
            if block % 2 == 0 {
                shared.pull(&mut actual);
            } else {
                original.pull(&mut actual);
            }
            assert!(expected.iter().any(|x| x.abs() > 0.001));
            assert_eq!(actual, expected, "sr={sr}, block={block}");
        }
        drop(original);
        baseline.pull(&mut expected);
        shared.pull(&mut actual);
        assert_eq!(
            actual, expected,
            "shared playback must outlive its original handle"
        );
    }
}

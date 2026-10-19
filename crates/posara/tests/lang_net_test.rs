#![cfg(all(feature = "net", feature = "compiler", feature = "gfx"))]

// End-to-end native glue: a cart drives the registered udp_* natives through a
// real VM + heap over loopback. Covers udp_send (read_string addr + Array<Int>
// buffer), udp_recv (alloc_bytes back into the heap), and udp_bind — the wiring
// the socket-table unit tests bypass.

use std::io::Write;
use std::net::UdpSocket;
use std::time::Duration;

use posara::runner::{compile_abe, Stepper};
use posara::Host;

#[test]
fn cart_udp_send_recv_roundtrip() {
    let listener = UdpSocket::bind("127.0.0.1:0").unwrap();
    listener.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let port = listener.local_addr().unwrap().port();

    // frame 1: cart binds, sends "hi" to the listener. frame 2: cart reads the
    // listener's reply byte and paints it across the screen.
    let src = format!(
        "@cart\nfn main() -> <frame, Graphics, IO, net> Unit {{\n  \
           let s = udp_bind(0);\n  \
           let msg = [104, 105];\n  \
           let _ = udp_send(s, \"127.0.0.1:{port}\", &msg);\n  \
           gfx_screen(4, 4);\n  \
           loop {{\n    \
             let b = udp_recv(s);\n    \
             if b.len() > 0 {{ gfx_cls(b.byte_at(0)) }} else {{ gfx_cls(0) }};\n    \
             gfx_commit();\n    frame.present()\n  \
           }}\n}}\n"
    );

    let root = std::env::temp_dir().join("posara_net_rt");
    std::fs::create_dir_all(&root).unwrap();
    let cart = root.join("net.abe");
    std::fs::File::create(&cart).unwrap().write_all(src.as_bytes()).unwrap();

    let host = Host::new_with(root.clone(), true, true).unwrap();
    let r = compile_abe(&cart, &host).unwrap();
    let mut step = Stepper::start_named(r.module, r.static_names, r.fn_names, &host).unwrap();

    step.frame().unwrap();   // cart sends "hi"

    let mut buf = [0u8; 64];
    let (n, from) = listener.recv_from(&mut buf).expect("listener got no datagram (udp_send glue)");
    assert_eq!(&buf[..n], b"hi", "udp_send delivered wrong bytes");

    listener.send_to(&[0x33], from).unwrap();
    // poll a few frames for the reply to arrive (nonblocking recv, no hang).
    for _ in 0..200 {
        step.frame().unwrap();
        if host.gfx.fb.borrow().buf.iter().all(|&c| c == 0x33) { break; }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(
        host.gfx.fb.borrow().buf.iter().all(|&c| c == 0x33),
        "udp_recv glue: reply byte 0x33 never painted"
    );

    std::fs::remove_dir_all(&root).ok();
}

// Discovery through the VM: cart announces, learns the peer's routable address
// from the reply via udp_from, and answers there. Exercises udp_from + the
// cart↔cart reply flow the socket oracles cover only at the Rust layer.
#[test]
fn cart_discovery_reply_via_from() {
    let peer = UdpSocket::bind("127.0.0.1:0").unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let port = peer.local_addr().unwrap().port();

    let src = format!(
        "@cart\nfn main() -> <frame, Graphics, IO, net> Unit {{\n  \
           let s = udp_bind(0);\n  \
           let hello = [1];\n  \
           let _ = udp_send(s, \"127.0.0.1:{port}\", &hello);\n  \
           gfx_screen(4, 4);\n  \
           loop {{\n    \
             let b = udp_recv(s);\n    \
             if b.len() > 0 {{\n      \
               let from = udp_from(s);\n      \
               let pong = [112, 111, 110, 103];\n      \
               let _ = udp_send(s, from, &pong)\n    \
             }};\n    \
             gfx_cls(0); gfx_commit(); frame.present()\n  \
           }}\n}}\n"
    );

    let root = std::env::temp_dir().join("posara_net_disc");
    std::fs::create_dir_all(&root).unwrap();
    let cart = root.join("d.abe");
    std::fs::File::create(&cart).unwrap().write_all(src.as_bytes()).unwrap();

    let host = Host::new_with(root.clone(), true, true).unwrap();
    let r = compile_abe(&cart, &host).unwrap();
    let mut step = Stepper::start_named(r.module, r.static_names, r.fn_names, &host).unwrap();

    step.frame().unwrap();                                  // cart announces
    let mut buf = [0u8; 16];
    let (_, cart_addr) = peer.recv_from(&mut buf).expect("no announce");
    peer.send_to(b"ping", cart_addr).unwrap();             // trigger the reply path

    peer.set_nonblocking(true).unwrap();
    let mut got = None;
    for _ in 0..300 {
        step.frame().unwrap();
        if let Ok((n, _)) = peer.recv_from(&mut buf) { got = Some(buf[..n].to_vec()); break; }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(got.as_deref(), Some(&b"pong"[..]), "cart did not reply to the learned peer addr");

    std::fs::remove_dir_all(&root).ok();
}

// A cart-built OSC packet must be byte-exact to the OSC 1.0 wire layout. The
// encoder + s_new builder are self-contained fixtures here — the test owns its
// inputs. Asserts the datagram matches the hand-computed /s_new bytes.
#[test]
fn osc_s_new_bytes_exact() {
    let listener = UdpSocket::bind("127.0.0.1:0").unwrap();
    listener.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let port = listener.local_addr().unwrap().port();

    let root = std::env::temp_dir().join("posara_osc_snew");
    std::fs::create_dir_all(&root).unwrap();

    // OSC encoder: str (null-padded to 4) + big-endian i32 into a caller Array<Int>.
    let osc = "pub fn osc_str(buf: &mut Array<Int>, off: Int, s: String) -> Int {\n  \
                 let n = s.len();\n  \
                 for i in 0..n { buf[off + i] = s.byte_at(i) };\n  \
                 off + (n / 4 + 1) * 4\n}\n\
               pub fn osc_i32(buf: &mut Array<Int>, off: Int, v: Int) -> Int {\n  \
                 buf[off] = (v >> 24) & 0xFF;\n  \
                 buf[off + 1] = (v >> 16) & 0xFF;\n  \
                 buf[off + 2] = (v >> 8) & 0xFF;\n  \
                 buf[off + 3] = v & 0xFF;\n  \
                 off + 4\n}\n";
    std::fs::write(root.join("osc.abe"), osc).unwrap();

    // /s_new "default" -1 0 1 "freq" <freq>, built at fn top level (borrow release).
    let sc = "use osc::{osc_str, osc_i32}\n\
              pub fn s_new(freq: Int) -> Array<Int> {\n  \
                let mut buf = [0; 48];\n  \
                let mut n = 0;\n  \
                n = osc_str(&mut buf, n, \"/s_new\");\n  \
                n = osc_str(&mut buf, n, \",siiisi\");\n  \
                n = osc_str(&mut buf, n, \"default\");\n  \
                n = osc_i32(&mut buf, n, -1);\n  \
                n = osc_i32(&mut buf, n, 0);\n  \
                n = osc_i32(&mut buf, n, 1);\n  \
                n = osc_str(&mut buf, n, \"freq\");\n  \
                let _ = osc_i32(&mut buf, n, freq);\n  \
                buf\n}\n";
    std::fs::write(root.join("sc.abe"), sc).unwrap();

    let src = format!(
        "use sc::{{s_new}}\n@cart\n\
         fn main() -> <frame, Graphics, IO, net> Unit {{\n  \
           let s = udp_bind(0);\n  \
           let pkt = s_new(440);\n  \
           let _ = udp_send(s, \"127.0.0.1:{port}\", &pkt);\n  \
           gfx_screen(4, 4);\n  \
           loop {{ gfx_cls(0); gfx_commit(); frame.present() }}\n}}\n"
    );
    let cart = root.join("send.abe");
    std::fs::File::create(&cart).unwrap().write_all(src.as_bytes()).unwrap();

    let host = Host::new_with(root.clone(), true, true).unwrap();
    let r = compile_abe(&cart, &host).unwrap();
    let mut step = Stepper::start_named(r.module, r.static_names, r.fn_names, &host).unwrap();
    step.frame().unwrap();

    let mut buf = [0u8; 128];
    let (n, _) = listener.recv_from(&mut buf).expect("no OSC datagram");

    let mut want = Vec::new();
    want.extend_from_slice(b"/s_new\0\0");
    want.extend_from_slice(b",siiisi\0");
    want.extend_from_slice(b"default\0");
    want.extend_from_slice(&(-1i32).to_be_bytes());
    want.extend_from_slice(&0i32.to_be_bytes());
    want.extend_from_slice(&1i32.to_be_bytes());
    want.extend_from_slice(b"freq\0\0\0\0");
    want.extend_from_slice(&440i32.to_be_bytes());
    assert_eq!(&buf[..n], &want[..], "OSC /s_new bytes mismatch");

    std::fs::remove_dir_all(&root).ok();
}

// Using a stale socket (bound, closed, then sent on) is a loud error, not a
// silent no-op — the observability the -1 sentinel lacks. The error surfaces on
// load or on the frame that runs it; either way the cart can't proceed unaware.
#[test]
fn cart_stale_socket_errors() {
    let src = "@cart\nfn main() -> <frame, Graphics, IO, net> Unit {\n  \
        let s = udp_bind(0);\n  \
        let _ = udp_close(s);\n  \
        let buf = [1];\n  \
        gfx_screen(4, 4);\n  \
        loop { let _ = udp_send(s, \"127.0.0.1:9999\", &buf); gfx_cls(0); gfx_commit(); frame.present() }\n}\n";
    let root = std::env::temp_dir().join("posara_net_stale");
    std::fs::create_dir_all(&root).unwrap();
    let cart = root.join("b.abe");
    std::fs::File::create(&cart).unwrap().write_all(src.as_bytes()).unwrap();

    let host = Host::new_with(root.clone(), true, true).unwrap();
    let run = || -> Result<(), String> {
        let r = compile_abe(&cart, &host)?;
        let mut step = Stepper::start_named(r.module, r.static_names, r.fn_names, &host)?;
        step.frame()?;
        Ok(())
    };
    let err = run().expect_err("stale socket must raise an error, not a silent no-op");
    assert!(err.contains("invalid socket"), "expected invalid-socket error, got: {err}");

    std::fs::remove_dir_all(&root).ok();
}

fn compile_str(name: &str, src: &str) -> Result<(), String> {
    let root = std::env::temp_dir().join(name);
    std::fs::create_dir_all(&root).unwrap();
    let cart = root.join("c.abe");
    std::fs::File::create(&cart).unwrap().write_all(src.as_bytes()).unwrap();
    let host = Host::new_with(root.clone(), true, true).unwrap();
    let r = compile_abe(&cart, &host).map(|_| ());
    std::fs::remove_dir_all(&root).ok();
    r
}

// effect_required: a pure-signature fn calling a <net> native must be rejected —
// net is tracked, not ambient. This is the guard that keeps IO/net distinct.
#[test]
fn calling_udp_without_net_effect_rejected() {
    let src = "@cart\nfn main() -> <frame, Graphics, IO> Unit {\n  \
        let _ = udp_bind(0);\n  \
        gfx_screen(4, 4);\n  \
        loop { gfx_cls(0); gfx_commit(); frame.present() }\n}\n";
    let err = compile_str("posara_net_noeff", src).unwrap_err();
    assert!(err.contains("net"), "expected net-effect error, got: {err}");
}

// unknown_effect_errors: naming an effect no host declares is a compile error,
// not a silently-dropped pure fn.
#[test]
fn unknown_effect_rejected() {
    let src = "@cart\nfn main() -> <frame, Graphics, IO, bogus> Unit {\n  \
        gfx_screen(4, 4);\n  \
        loop { gfx_cls(0); gfx_commit(); frame.present() }\n}\n";
    assert!(compile_str("posara_net_bogus", src).is_err(), "unknown effect must error");
}

// net IS dischargeable by @cart main once the host declares udp_* — the positive
// case for the effect the two tests above reject.
#[test]
fn cart_with_net_effect_compiles() {
    let src = "@cart\nfn main() -> <frame, Graphics, IO, net> Unit {\n  \
        let _ = udp_bind(0);\n  \
        gfx_screen(4, 4);\n  \
        loop { gfx_cls(0); gfx_commit(); frame.present() }\n}\n";
    compile_str("posara_net_ok", src).expect("net cart should compile");
}

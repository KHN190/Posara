#![cfg(feature = "gfx")]

use posara::{Button, Controller};

// controller bitmap contract (port 0x82): exact bit per button.
#[test]
fn button_bitmap_values() {
    assert_eq!(Button::A as u8, 0x01);
    assert_eq!(Button::B as u8, 0x02);
    assert_eq!(Button::Select as u8, 0x04);
    assert_eq!(Button::Start as u8, 0x08);
    assert_eq!(Button::Up as u8, 0x10);
    assert_eq!(Button::Down as u8, 0x20);
    assert_eq!(Button::Left as u8, 0x40);
    assert_eq!(Button::Right as u8, 0x80);
}

#[test]
fn buttons_are_disjoint_bits() {
    let all = (Button::A as u8) | (Button::B as u8) | (Button::Select as u8)
        | (Button::Start as u8) | (Button::Up as u8) | (Button::Down as u8)
        | (Button::Left as u8) | (Button::Right as u8);
    assert_eq!(all, 0xFF);
}

#[test]
fn new_controller_is_clear() {
    let c = Controller::new();
    assert_eq!(c.buttons, 0);
    assert_eq!(c.key, 0);
}

#[cfg(feature = "compiler")]
fn run_one_frame(tag: &str, src: &str, setup: impl FnOnce(&posara::Host)) -> Vec<u16> {
    use std::io::Write;
    use posara::Host;
    use posara::runner::{compile_abe, Stepper};

    let mut p = std::env::temp_dir();
    p.push(format!("posara_{tag}.abe"));
    std::fs::File::create(&p).unwrap().write_all(src.as_bytes()).unwrap();

    let host = Host::new_with(std::path::PathBuf::from("."), true, true).unwrap();
    let r = compile_abe(&p, &host).unwrap();
    setup(&host);

    let mut step = Stepper::start_named(r.module, r.static_names, r.fn_names, &host).unwrap();
    step.frame().unwrap();
    let buf = host.gfx.fb.borrow().buf.clone();
    std::fs::remove_file(&p).ok();
    buf
}

#[cfg(feature = "compiler")]
#[test]
fn in_buttons_reads_injected_state() {
    let src = "@cart\nfn main() -> <frame, Graphics, IO> Unit {\n  gfx_screen(8, 8);\n  loop { gfx_cls(in_buttons()); gfx_commit(); frame.present() }\n}\n";
    let buf = run_one_frame("in_buttons", src, |h| h.input.controller.borrow_mut().buttons = 0x42);
    assert!(buf.iter().all(|&c| c == 0x42));
}

#[cfg(feature = "compiler")]
#[test]
fn gfx_prefixed_natives_render() {
    let src = "@cart\nfn main() -> <frame, Graphics, IO> Unit {\n  gfx_screen(8, 8);\n  loop { gfx_cls(0x1234); gfx_commit(); frame.present() }\n}\n";
    let buf = run_one_frame("gfx_alias", src, |_| {});
    assert!(buf.iter().all(|&c| c == 0x1234));
}

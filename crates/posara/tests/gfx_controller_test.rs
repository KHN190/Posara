#![cfg(feature = "gfx")]

use posara::{Button, Controller};

// controller bitmap contract (port 0x82): exact bit per button.
#[test]
fn button_bitmap_values_and_initial_state() {
    let bits = [
        (Button::A as u8, 0x01u8), (Button::B as u8, 0x02), (Button::Select as u8, 0x04), (Button::Start as u8, 0x08),
        (Button::Up as u8, 0x10), (Button::Down as u8, 0x20), (Button::Left as u8, 0x40), (Button::Right as u8, 0x80),
    ];
    for (b, want) in bits { assert_eq!(b, want); }
    assert_eq!(bits.iter().fold(0u8, |acc, &(b, _)| acc | b), 0xFF);
    let c = Controller::new();
    assert_eq!((c.buttons, c.key), (0, 0));
}

#[cfg(feature = "compiler")]
mod every_frame {
    use std::io::Write;

    use posara::runner::{compile_abe, Stepper};
    use posara::Host;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn the_cart_reads_exactly_the_buttons_and_key_of_each_frame(inputs in prop::collection::vec((any::<u8>(), any::<u8>()), 1..40)) {
            let src = "@cart\nfn main() -> <frame, Graphics, IO> Unit {\n  gfx_screen(2, 1);\n  loop {\n    gfx_rect(0, 0, 1, 1, in_buttons());\n    gfx_rect(1, 0, 1, 1, in_key());\n    gfx_commit();\n    frame.present()\n  }\n}\n";
            let p = std::env::temp_dir().join(format!("posara_inputs_{}.abe", std::process::id()));
            std::fs::File::create(&p).unwrap().write_all(src.as_bytes()).unwrap();
            let host = Host::new_with(std::path::PathBuf::from("."), true, true).unwrap();
            let r = compile_abe(&p, &host).unwrap();
            let mut step = Stepper::start_named(r.module, r.static_names, r.fn_names, &host).unwrap();
            for (b, k) in inputs {
                host.set_input(b, k);
                step.frame().unwrap();
                prop_assert_eq!(host.gfx.fb.borrow().buf.clone(), vec![b as u16, k as u16]);
            }
        }
    }
}

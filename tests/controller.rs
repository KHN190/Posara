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

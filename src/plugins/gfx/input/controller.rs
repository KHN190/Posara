use minifb::{Key, KeyRepeat, Window};

#[repr(u8)]
pub enum Button {
    A      = 0x01,
    B      = 0x02,
    Select = 0x04,
    Start  = 0x08,
    Up     = 0x10,
    Down   = 0x20,
    Left   = 0x40,
    Right  = 0x80,
}

#[derive(Default)]
pub struct Controller {
    pub buttons: u8,
    pub key: u8,
}

impl Controller {
    pub fn new() -> Self { Self::default() }

    pub fn poll(&mut self, win: &Window) {
        let mut b = 0u8;
        if win.is_key_down(Key::Z)     { b |= Button::A as u8; }
        if win.is_key_down(Key::X)     { b |= Button::B as u8; }
        if win.is_key_down(Key::Tab)   { b |= Button::Select as u8; }
        if win.is_key_down(Key::Enter) { b |= Button::Start as u8; }
        if win.is_key_down(Key::Up)    { b |= Button::Up as u8; }
        if win.is_key_down(Key::Down)  { b |= Button::Down as u8; }
        if win.is_key_down(Key::Left)  { b |= Button::Left as u8; }
        if win.is_key_down(Key::Right) { b |= Button::Right as u8; }
        self.buttons = b;

        self.key = win.get_keys_pressed(KeyRepeat::No)
            .into_iter()
            .find_map(ascii_of_key)
            .unwrap_or(0);
    }
}

fn ascii_of_key(k: Key) -> Option<u8> {
    use Key::*;
    let c = match k {
        A=>'a', B=>'b', C=>'c', D=>'d', E=>'e', F=>'f', G=>'g', H=>'h',
        I=>'i', J=>'j', K=>'k', L=>'l', M=>'m', N=>'n', O=>'o', P=>'p',
        Q=>'q', R=>'r', S=>'s', T=>'t', U=>'u', V=>'v', W=>'w', X=>'x',
        Y=>'y', Z=>'z',
        Key0=>'0', Key1=>'1', Key2=>'2', Key3=>'3', Key4=>'4',
        Key5=>'5', Key6=>'6', Key7=>'7', Key8=>'8', Key9=>'9',
        Space=>' ',
        _ => return None,
    };
    Some(c as u8)
}

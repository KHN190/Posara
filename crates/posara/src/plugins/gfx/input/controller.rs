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

    pub fn set(&mut self, buttons: u8, key: u8) {
        self.buttons = buttons;
        self.key = key;
    }
}

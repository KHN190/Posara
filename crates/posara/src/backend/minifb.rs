use minifb::{Key, KeyRepeat, Window, WindowOptions};

use super::Presenter;
use crate::plugins::gfx::framebuffer::rgb565_to_rgb888;
use crate::plugins::gfx::input::Button;

pub struct MinifbPresenter {
    win: Option<Window>,
    out: Vec<u32>,
    w: usize,
    h: usize,
}

impl MinifbPresenter {
    pub fn new() -> Self {
        Self { win: None, out: vec![], w: 0, h: 0 }
    }
}

impl Presenter for MinifbPresenter {
    fn configure(&mut self, w: usize, h: usize) -> Result<(), String> {
        if self.win.is_some() { return Ok(()); }
        let win = Window::new("posara", w, h, WindowOptions::default()).map_err(|e| e.to_string())?;
        self.win = Some(win);
        self.out = vec![0; w * h];
        self.w = w;
        self.h = h;
        Ok(())
    }

    fn present(&mut self, buf: &[u16]) -> Result<bool, String> {
        let Some(win) = self.win.as_mut() else { return Ok(true); };
        if !win.is_open() { return Ok(false); }
        for (i, &p) in buf.iter().enumerate() { self.out[i] = rgb565_to_rgb888(p); }
        win.update_with_buffer(&self.out, self.w, self.h).map_err(|e| e.to_string())?;
        Ok(true)
    }

    fn poll(&mut self) -> Option<(u8, u8)> {
        let win = self.win.as_ref()?;
        let mut b = 0u8;
        if win.is_key_down(Key::Z)     { b |= Button::A as u8; }
        if win.is_key_down(Key::X)     { b |= Button::B as u8; }
        if win.is_key_down(Key::Tab)   { b |= Button::Select as u8; }
        if win.is_key_down(Key::Enter) { b |= Button::Start as u8; }
        if win.is_key_down(Key::Up)    { b |= Button::Up as u8; }
        if win.is_key_down(Key::Down)  { b |= Button::Down as u8; }
        if win.is_key_down(Key::Left)  { b |= Button::Left as u8; }
        if win.is_key_down(Key::Right) { b |= Button::Right as u8; }
        let key = win.get_keys_pressed(KeyRepeat::No).into_iter().find_map(ascii_of_key).unwrap_or(0);
        Some((b, key))
    }

    fn set_pos(&mut self, x: i64, y: i64) {
        if let Some(w) = self.win.as_mut() { w.set_position(x as isize, y as isize); }
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

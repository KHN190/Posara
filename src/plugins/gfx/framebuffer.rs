use minifb::{Window, WindowOptions};

pub struct Framebuffer {
    pub w: usize,
    pub h: usize,
    pub format: u8,
    pub buf: Vec<u16>,
    pub out: Vec<u32>,
    pub window: Option<Window>,
    pub alive: bool,
    pub headless: bool,
    pub palette: [u16; 16],
    pub commits: u64,
}

impl Framebuffer {
    pub fn new() -> Self {
        Self { w: 0, h: 0, format: 0, buf: vec![], out: vec![], window: None, alive: true, headless: false, palette: [0; 16], commits: 0 }
    }

    // Must be called before configure() to take effect; otherwise the window
    // is already open and we leave it alone (warn).
    pub fn set_headless(&mut self) {
        if self.window.is_some() {
            eprintln!("screen_off: window already open (screen_off must precede screen); ignored");
            return;
        }
        self.headless = true;
    }

    pub fn configure(&mut self, w: usize, h: usize, format: u8) -> Result<(), String> {
        if format != 1 { return Err(format!("Screen: format {} unsupported (only 1=RGB565)", format)); }
        if w == 0 || h == 0 { return Err(format!("Screen: invalid size {}x{}", w, h)); }
        if self.window.is_some() || (self.headless && !self.buf.is_empty()) {
            if self.w != w || self.h != h {
                return Err("Screen: reconfigure with different size not supported".into());
            }
            return Ok(());
        }
        if !self.headless {
            let win = Window::new("posara", w, h, WindowOptions::default())
                .map_err(|e| e.to_string())?;
            self.window = Some(win);
        }
        self.w = w;
        self.h = h;
        self.format = format;
        self.buf = vec![0; w * h];
        self.out = vec![0; w * h];
        Ok(())
    }

    pub fn cls(&mut self, c: u16) {
        for px in self.buf.iter_mut() { *px = c; }
    }

    pub fn pset(&mut self, x: i64, y: i64, c: u16) {
        if x < 0 || y < 0 { return; }
        let (x, y) = (x as usize, y as usize);
        if x >= self.w || y >= self.h { return; }
        self.buf[y * self.w + x] = c;
    }

    // Composite a pixel under a blit mode: 0 REPLACE, 1 XOR, 2 AND, 3 OR.
    // bit=0 source pixels never reach here (caller masks); this is the bit=1 op.
    pub fn pset_op(&mut self, x: i64, y: i64, c: u16, mode: i64) {
        if x < 0 || y < 0 { return; }
        let (x, y) = (x as usize, y as usize);
        if x >= self.w || y >= self.h { return; }
        let i = y * self.w + x;
        self.buf[i] = match mode {
            1 => self.buf[i] ^ c,
            2 => self.buf[i] & c,
            3 => self.buf[i] | c,
            _ => c,
        };
    }

    // Alpha-blend `c` over existing pixels in the rect (a: 0..256). Reads the
    // framebuffer so overlays mix with whatever is already drawn underneath.
    pub fn rect_mix(&mut self, x0: i64, y0: i64, w: i64, h: i64, c: u16, a: i64) {
        let a = a.clamp(0, 256) as u32;
        if a == 0 { return; }
        for dy in 0..h {
            for dx in 0..w {
                let (x, y) = (x0 + dx, y0 + dy);
                if x < 0 || y < 0 { continue; }
                let (x, y) = (x as usize, y as usize);
                if x >= self.w || y >= self.h { continue; }
                let idx = y * self.w + x;
                self.buf[idx] = blend565(self.buf[idx], c, a);
            }
        }
    }

    // Floyd-Steinberg dither the whole framebuffer to two colors by luminance.
    // 1-bit when called dither(0x0000, 0xFFFF). One Rust pass — fast.
    pub fn dither(&mut self, dark: u16, light: u16) {
        let (w, h) = (self.w, self.h);
        if w == 0 || h == 0 { return; }
        let mut lum: Vec<i32> = self.buf.iter().map(|&c| {
            let r = ((c >> 11) & 0x1F) as i32 * 255 / 31;
            let g = ((c >> 5)  & 0x3F) as i32 * 255 / 63;
            let b = ( c        & 0x1F) as i32 * 255 / 31;
            (r * 77 + g * 150 + b * 29) >> 8
        }).collect();
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let old = lum[i];
                let on = old >= 128;
                let err = old - if on { 255 } else { 0 };
                self.buf[i] = if on { light } else { dark };
                if x + 1 < w { lum[i + 1] += err * 7 / 16; }
                if y + 1 < h {
                    if x > 0 { lum[i + w - 1] += err * 3 / 16; }
                    lum[i + w] += err * 5 / 16;
                    if x + 1 < w { lum[i + w + 1] += err / 16; }
                }
            }
        }
    }

    pub fn commit(&mut self) -> Result<(), String> {
        self.commits += 1;
        let Some(win) = self.window.as_mut() else { return Ok(()); };
        self.alive = win.is_open();
        if self.alive {
            for (i, &p) in self.buf.iter().enumerate() {
                self.out[i] = rgb565_to_rgb888(p);
            }
            win.update_with_buffer(&self.out, self.w, self.h).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

fn blend565(d: u16, s: u16, a: u32) -> u16 {
    let inv = 256 - a;
    let dr = ((d >> 11) & 0x1F) as u32;
    let dg = ((d >> 5)  & 0x3F) as u32;
    let db = ( d        & 0x1F) as u32;
    let sr = ((s >> 11) & 0x1F) as u32;
    let sg = ((s >> 5)  & 0x3F) as u32;
    let sb = ( s        & 0x1F) as u32;
    let r = (dr * inv + sr * a) / 256;
    let g = (dg * inv + sg * a) / 256;
    let b = (db * inv + sb * a) / 256;
    ((r << 11) | (g << 5) | b) as u16
}

fn rgb565_to_rgb888(c: u16) -> u32 {
    let r = ((c >> 11) & 0x1F) as u32;
    let g = ((c >> 5)  & 0x3F) as u32;
    let b = ( c        & 0x1F) as u32;
    let r8 = (r * 255 / 31) << 16;
    let g8 = (g * 255 / 63) << 8;
    let b8 =  b * 255 / 31;
    r8 | g8 | b8
}

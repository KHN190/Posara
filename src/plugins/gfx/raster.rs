use super::Framebuffer;

fn edge(ax: i64, ay: i64, bx: i64, by: i64, px: i64, py: i64) -> i64 {
    (bx - ax) * (py - ay) - (by - ay) * (px - ax)
}

// Cohen–Sutherland clip to [0,w)×[0,h). 
const CLIP_LIM: i64 = 1 << 20;

fn outcode(x: i64, y: i64, w: i64, h: i64) -> u8 {
    let mut c = 0u8;
    if x < 0 { c |= 1; } else if x >= w { c |= 2; }
    if y < 0 { c |= 4; } else if y >= h { c |= 8; }
    c
}

fn clip(mut x0: i64, mut y0: i64, mut x1: i64, mut y1: i64, w: i64, h: i64) -> Option<(i64, i64, i64, i64)> {
    if w <= 0 || h <= 0 { return None; }
    if x0.abs() > CLIP_LIM || y0.abs() > CLIP_LIM || x1.abs() > CLIP_LIM || y1.abs() > CLIP_LIM {
        return None;
    }
    let mut c0 = outcode(x0, y0, w, h);
    let mut c1 = outcode(x1, y1, w, h);
    loop {
        if c0 | c1 == 0 { return Some((x0, y0, x1, y1)); }
        if c0 & c1 != 0 { return None; }
        let c = if c0 != 0 { c0 } else { c1 };
        let (x, y);
        if c & 8 != 0 { x = x0 + (x1 - x0) * (h - 1 - y0) / (y1 - y0); y = h - 1; }
        else if c & 4 != 0 { x = x0 + (x1 - x0) * (0 - y0) / (y1 - y0); y = 0; }
        else if c & 2 != 0 { y = y0 + (y1 - y0) * (w - 1 - x0) / (x1 - x0); x = w - 1; }
        else { y = y0 + (y1 - y0) * (0 - x0) / (x1 - x0); x = 0; }
        if c == c0 { x0 = x; y0 = y; c0 = outcode(x0, y0, w, h); }
        else { x1 = x; y1 = y; c1 = outcode(x1, y1, w, h); }
    }
}

impl Framebuffer {
    pub fn line(&mut self, x0: i64, y0: i64, x1: i64, y1: i64, c: u16) {
        let Some((x0, y0, x1, y1)) = clip(x0, y0, x1, y1, self.w as i64, self.h as i64) else { return; };
        let (mut x, mut y) = (x0, y0);
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.pset(x, y, c);
            if x == x1 && y == y1 { break; }
            let e2 = 2 * err;
            if e2 >= dy { err += dy; x += sx; }
            if e2 <= dx { err += dx; y += sy; }
        }
    }

    pub fn line_thick(&mut self, x0: i64, y0: i64, x1: i64, y1: i64, t: i64, c: u16) {
        let t = t.max(1);
        if t == 1 { return self.line(x0, y0, x1, y1, c); }
        let Some((x0, y0, x1, y1)) = clip(x0, y0, x1, y1, self.w as i64, self.h as i64) else { return; };
        let off = t / 2;
        let (mut x, mut y) = (x0, y0);
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            for oy in 0..t {
                for ox in 0..t {
                    self.pset(x - off + ox, y - off + oy, c);
                }
            }
            if x == x1 && y == y1 { break; }
            let e2 = 2 * err;
            if e2 >= dy { err += dy; x += sx; }
            if e2 <= dx { err += dx; y += sy; }
        }
    }

    pub fn rect_outline(&mut self, x: i64, y: i64, w: i64, h: i64, c: u16) {
        if w <= 0 || h <= 0 { return; }
        self.line(x, y, x + w - 1, y, c);
        self.line(x, y + h - 1, x + w - 1, y + h - 1, c);
        self.line(x, y, x, y + h - 1, c);
        self.line(x + w - 1, y, x + w - 1, y + h - 1, c);
    }

    pub fn circ(&mut self, cx: i64, cy: i64, r: i64, c: u16, fill: bool) {
        if r < 0 { return; }
        if fill {
            for dy in -r..=r {
                let dx = (((r * r - dy * dy) as f64).sqrt()) as i64;
                self.line(cx - dx, cy + dy, cx + dx, cy + dy, c);
            }
            return;
        }
        let (mut x, mut y, mut d) = (r, 0i64, 1 - r);
        while x >= y {
            self.pset(cx + x, cy + y, c); self.pset(cx + y, cy + x, c);
            self.pset(cx - y, cy + x, c); self.pset(cx - x, cy + y, c);
            self.pset(cx - x, cy - y, c); self.pset(cx - y, cy - x, c);
            self.pset(cx + y, cy - x, c); self.pset(cx + x, cy - y, c);
            y += 1;
            if d < 0 { d += 2 * y + 1; } else { x -= 1; d += 2 * (y - x) + 1; }
        }
    }

    pub fn tri(&mut self, x0: i64, y0: i64, x1: i64, y1: i64, x2: i64, y2: i64, c: u16, fill: bool) {
        if !fill {
            self.line(x0, y0, x1, y1, c);
            self.line(x1, y1, x2, y2, c);
            self.line(x2, y2, x0, y0, c);
            return;
        }
        let minx = x0.min(x1).min(x2).max(0);
        let miny = y0.min(y1).min(y2).max(0);
        let maxx = x0.max(x1).max(x2).min(self.w as i64 - 1);
        let maxy = y0.max(y1).max(y2).min(self.h as i64 - 1);
        for py in miny..=maxy {
            for px in minx..=maxx {
                let w0 = edge(x1, y1, x2, y2, px, py);
                let w1 = edge(x2, y2, x0, y0, px, py);
                let w2 = edge(x0, y0, x1, y1, px, py);
                if (w0 >= 0 && w1 >= 0 && w2 >= 0) || (w0 <= 0 && w1 <= 0 && w2 <= 0) {
                    self.pset(px, py, c);
                }
            }
        }
    }
}

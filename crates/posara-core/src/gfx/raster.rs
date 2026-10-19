use super::Framebuffer;

fn edge(ax: i64, ay: i64, bx: i64, by: i64, px: i64, py: i64) -> i128 {
    (bx as i128 - ax as i128) * (py as i128 - ay as i128) - (by as i128 - ay as i128) * (px as i128 - ax as i128)
}

const CLIP_LIM: i64 = 1 << 20;

fn outcode(x: i64, y: i64, w: i64, h: i64) -> u8 {
    let mut c = 0u8;
    if x < 0 { c |= 1; } else if x >= w { c |= 2; }
    if y < 0 { c |= 4; } else if y >= h { c |= 8; }
    c
}

impl Framebuffer {
    // Walks the unclipped line so the visible pixels never depend on where it leaves the screen.
    fn walk(&mut self, x0: i64, y0: i64, x1: i64, y1: i64, m: i64, mut f: impl FnMut(&mut Self, i64, i64)) {
        let (w, h) = (self.w as i64 + 2 * m, self.h as i64 + 2 * m);
        if [x0, y0, x1, y1].iter().any(|v| v.abs() > CLIP_LIM) { return; }
        if outcode(x0 + m, y0 + m, w, h) & outcode(x1 + m, y1 + m, w, h) != 0 { return; }
        let inside = |x: i64, y: i64| x >= -m && y >= -m && x < w - m && y < h - m;
        let (mut x, mut y) = (x0, y0);
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let mut seen = false;
        loop {
            if inside(x, y) {
                seen = true;
                f(self, x, y);
            } else if seen {
                break;
            }
            if x == x1 && y == y1 { break; }
            let e2 = 2 * err;
            if e2 >= dy { err += dy; x += sx; }
            if e2 <= dx { err += dx; y += sy; }
        }
    }

    pub fn line(&mut self, x0: i64, y0: i64, x1: i64, y1: i64, c: u16) {
        self.walk(x0, y0, x1, y1, 0, |f, x, y| f.pset(x, y, c));
    }

    pub fn line_thick(&mut self, x0: i64, y0: i64, x1: i64, y1: i64, t: i64, c: u16) {
        let t = t.max(1);
        let off = t / 2;
        let (w, h) = (self.w as i64, self.h as i64);
        self.walk(x0, y0, x1, y1, t, |f, x, y| {
            for py in (y - off).max(0)..(y - off + t).min(h) {
                for px in (x - off).max(0)..(x - off + t).min(w) {
                    f.pset(px, py, c);
                }
            }
        });
    }

    pub fn rect_outline(&mut self, x: i64, y: i64, w: i64, h: i64, c: u16) {
        if w <= 0 || h <= 0 { return; }
        self.line(x, y, x + w - 1, y, c);
        self.line(x, y + h - 1, x + w - 1, y + h - 1, c);
        self.line(x, y, x, y + h - 1, c);
        self.line(x + w - 1, y, x + w - 1, y + h - 1, c);
    }

    pub fn circ(&mut self, cx: i64, cy: i64, r: i64, c: u16, fill: bool) {
        if !(0..=CLIP_LIM).contains(&r) || cx.abs() > CLIP_LIM || cy.abs() > CLIP_LIM { return; }
        if fill {
            // +r ≈ (r+0.5)² radius bias: fills the cardinal spikes the bare
            // dx²+dy²≤r² discretisation leaves on each axis, so the rim reads round.
            for dy in (-r).max(-cy)..=r.min(self.h as i64 - 1 - cy) {
                let dx = super::circle_span::<super::Real>(r, dy);
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

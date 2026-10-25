use super::Framebuffer;
use super::super::runtime::{Int, Word};

fn fraction_cmp(mut a: Word, mut b: Word, mut c: Word, mut d: Word) -> core::cmp::Ordering {
    let mut reverse = false;
    loop {
        let order = (a / b).cmp(&(c / d));
        if order != core::cmp::Ordering::Equal { return if reverse { order.reverse() } else { order }; }
        let (r,s) = (a % b, c % d);
        if r == 0 || s == 0 {
            let order = r.cmp(&s);
            return if reverse { order.reverse() } else { order };
        }
        (a,b,c,d) = (b,r,d,s);
        reverse = !reverse;
    }
}

fn product_cmp(a: Int, b: Int, c: Int, d: Int) -> core::cmp::Ordering {
    let (left_negative,right_negative) = ((a<0) != (b<0), (c<0) != (d<0));
    let (a,b,c,d) = (a.unsigned_abs(),b.unsigned_abs(),c.unsigned_abs(),d.unsigned_abs());
    let left_negative = left_negative && a != 0 && b != 0;
    let right_negative = right_negative && c != 0 && d != 0;
    if left_negative != right_negative { return right_negative.cmp(&left_negative); }
    let order = if a == 0 || b == 0 || c == 0 || d == 0 {
        (a != 0 && b != 0).cmp(&(c != 0 && d != 0))
    } else { fraction_cmp(a,c,d,b) };
    if left_negative { order.reverse() } else { order }
}

fn edge(ax: Int, ay: Int, bx: Int, by: Int, px: Int, py: Int) -> Int {
    let (a,b,c,d) = (bx-ax,py-ay,by-ay,px-ax);
    if let Some(value) = a.checked_mul(b).and_then(|left| c.checked_mul(d).and_then(|right|left.checked_sub(right))) { return value.signum(); }
    match product_cmp(a,b,c,d) { core::cmp::Ordering::Less => -1, core::cmp::Ordering::Equal => 0, core::cmp::Ordering::Greater => 1 }
}

fn outcode(x: Int, y: Int, w: Int, h: Int) -> u8 {
    let mut c = 0u8;
    if x < 0 { c |= 1; } else if x >= w { c |= 2; }
    if y < 0 { c |= 4; } else if y >= h { c |= 8; }
    c
}

impl Framebuffer {
    // Walks the unclipped line so the visible pixels never depend on where it leaves the screen.
    fn walk(&mut self, x0: Int, y0: Int, x1: Int, y1: Int, m: Int, mut f: impl FnMut(&mut Self, Int, Int)) {
        let (w, h) = (self.w as Int + 2 * m, self.h as Int + 2 * m);
        if outcode(x0 + m, y0 + m, w, h) & outcode(x1 + m, y1 + m, w, h) != 0 { return; }
        let inside = |x: Int, y: Int| x >= -m && y >= -m && x < w - m && y < h - m;
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

    pub fn line(&mut self, x0: Int, y0: Int, x1: Int, y1: Int, c: u16) {
        self.walk(x0, y0, x1, y1, 0, |f, x, y| f.pset(x, y, c));
    }

    pub fn line_thick(&mut self, x0: Int, y0: Int, x1: Int, y1: Int, t: Int, c: u16) {
        let t = t.max(1);
        let off = t / 2;
        let (w, h) = (self.w as Int, self.h as Int);
        self.walk(x0, y0, x1, y1, t, |f, x, y| {
            for py in (y - off).max(0)..(y - off + t).min(h) {
                for px in (x - off).max(0)..(x - off + t).min(w) {
                    f.pset(px, py, c);
                }
            }
        });
    }

    pub fn rect_outline(&mut self, x: Int, y: Int, w: Int, h: Int, c: u16) {
        if w <= 0 || h <= 0 { return; }
        self.line(x, y, x + w - 1, y, c);
        self.line(x, y + h - 1, x + w - 1, y + h - 1, c);
        self.line(x, y, x, y + h - 1, c);
        self.line(x + w - 1, y, x + w - 1, y + h - 1, c);
    }

    pub fn circ(&mut self, cx: Int, cy: Int, r: Int, c: u16, fill: bool) {
        if r < 0 { return; }
        if fill {
            // +r ≈ (r+0.5)² radius bias: fills the cardinal spikes the bare
            // dx²+dy²≤r² discretisation leaves on each axis, so the rim reads round.
            for dy in (-r).max(-cy)..=r.min(self.h as Int - 1 - cy) {
                let dx = super::circle_span(r, dy);
                self.line(cx - dx, cy + dy, cx + dx, cy + dy, c);
            }
            return;
        }
        let (mut x, mut y, mut d) = (r, 0 as Int, 1 - r);
        while x >= y {
            self.pset(cx + x, cy + y, c); self.pset(cx + y, cy + x, c);
            self.pset(cx - y, cy + x, c); self.pset(cx - x, cy + y, c);
            self.pset(cx - x, cy - y, c); self.pset(cx - y, cy - x, c);
            self.pset(cx + y, cy - x, c); self.pset(cx + x, cy - y, c);
            y += 1;
            if d < 0 { d += 2 * y + 1; } else { x -= 1; d += 2 * (y - x) + 1; }
        }
    }

    pub fn tri(&mut self, x0: Int, y0: Int, x1: Int, y1: Int, x2: Int, y2: Int, c: u16, fill: bool) {
        if !fill {
            self.line(x0, y0, x1, y1, c);
            self.line(x1, y1, x2, y2, c);
            self.line(x2, y2, x0, y0, c);
            return;
        }
        let minx = x0.min(x1).min(x2).max(0);
        let miny = y0.min(y1).min(y2).max(0);
        let maxx = x0.max(x1).max(x2).min(self.w as Int - 1);
        let maxy = y0.max(y1).max(y2).min(self.h as Int - 1);
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

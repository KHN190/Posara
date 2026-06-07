use std::collections::VecDeque;
use std::time::Duration;

use minifb::{Window, WindowOptions};

const PW: usize = 320;
const PH: usize = 264;
const FRAME_US: u64 = 16_667; // 60 Hz budget

const BG: u32 = 0x0008_0E0E;
const DIM: u32 = 0x0045_5252;
const INK: u32 = 0x005C_D6BE;
const HI: u32 = 0x00DC_E8E6;

// assets/mono8x16.fnt: cp 32..126, 16 bytes/glyph, 1 byte/row, MSB = leftmost.
static FONT: &[u8] = include_bytes!("../carts/assets/mono8x16.fnt");

pub struct Profiler {
    win: Window,
    buf: Vec<u32>,
    work_hist: VecDeque<u64>,
    fps: f64,
    work_us: u64,
    ops: u64,
    bytes: usize,
    cells: usize,
    peak_work_us: u64,
    peak_ops: u64,
    peak_bytes: usize,
    peak_cells: usize,
    notes: u32,
    out_peak: f32,
    ch_peak: [f32; 4],
    ch_voices: [u32; 4],
}

impl Profiler {
    pub fn new() -> Option<Self> {
        let win = Window::new("posara · profile", PW, PH, WindowOptions::default()).ok()?;
        Some(Self {
            win,
            buf: vec![BG; PW * PH],
            work_hist: VecDeque::with_capacity(PW),
            fps: 0.0,
            work_us: 0,
            ops: 0,
            bytes: 0,
            cells: 0,
            peak_work_us: 0,
            peak_ops: 0,
            peak_bytes: 0,
            peak_cells: 0,
            notes: 0,
            out_peak: 0.0,
            ch_peak: [0.0; 4],
            ch_voices: [0; 4],
        })
    }

    pub fn is_open(&self) -> bool { self.win.is_open() }

    pub fn sample(&mut self, ops: u64, bytes: usize, cells: usize, work: Duration, frame_dt: Duration) {
        self.ops = ops;
        self.bytes = bytes;
        self.cells = cells;
        self.work_us = work.as_micros() as u64;
        let dt = frame_dt.as_secs_f64();
        if dt > 0.0 { self.fps = 1.0 / dt; }
        self.peak_work_us = self.peak_work_us.max(self.work_us);
        self.peak_ops = self.peak_ops.max(ops);
        self.peak_bytes = self.peak_bytes.max(bytes);
        self.peak_cells = self.peak_cells.max(cells);
        if self.work_hist.len() == PW { self.work_hist.pop_front(); }
        self.work_hist.push_back(self.work_us);
    }

    pub fn set_audio(&mut self, notes: u32, out_peak: f32, ch_voices: [u32; 4], ch_peak: [f32; 4]) {
        self.notes = notes;
        self.ch_voices = ch_voices;
        self.out_peak = (self.out_peak * 0.90).max(out_peak);
        for i in 0..4 {
            self.ch_peak[i] = (self.ch_peak[i] * 0.90).max(ch_peak[i]);
        }
    }

    pub fn draw(&mut self) {
        for p in self.buf.iter_mut() { *p = BG; }

        let over = self.work_us > FRAME_US;
        let ms = self.work_us as f64 / 1000.0;
        let pms = self.peak_work_us as f64 / 1000.0;

        let rows: [(&str, String, &str, String, u32); 5] = [
            ("FPS", format!("{:.0}", self.fps), "fps", String::new(), HI),
            ("MS",  format!("{:.1}", ms), "ms", format!("{:.1}", pms), if over { HI } else { INK }),
            ("OPS", human(self.ops), "ops", human(self.peak_ops), HI),
            ("MEM", human(self.bytes as u64), "bytes", human(self.peak_bytes as u64), HI),
            ("OBJ", human(self.cells as u64), "obj", human(self.peak_cells as u64), HI),
        ];
        let mut y = 6;
        for (label, cur, unit, peak, color) in rows {
            text(&mut self.buf, 4, y, label, 1, DIM);
            text(&mut self.buf, 48, y, &format!("{} {}", cur, unit), 1, color);
            if !peak.is_empty() {
                text(&mut self.buf, 208, y, "PEAK", 1, DIM);
                text(&mut self.buf, 252, y, &peak, 1, DIM);
            }
            y += 18;
        }

        spark(&mut self.buf, &self.work_hist, self.peak_work_us, 100, 138);

        text(&mut self.buf, 4, 146, "HIT", 1, DIM);
        text(&mut self.buf, 56, 146, &human(self.notes as u64), 1, HI);

        let names = ["C0", "C1", "C2", "C3"];
        let mut cy = 168;
        for i in 0..4 {
            text(&mut self.buf, 4, cy, names[i], 1, DIM);
            text(&mut self.buf, 28, cy, &human(self.ch_voices[i] as u64), 1, HI);
            bar(&mut self.buf, 48, cy + 3, 200, 10, self.ch_peak[i]);
            cy += 18;
        }

        text(&mut self.buf, 4, 242, "OUT", 1, DIM);
        bar(&mut self.buf, 48, 245, 200, 10, self.out_peak);

        let _ = self.win.update_with_buffer(&self.buf, PW, PH);
    }
}

fn bar(buf: &mut [u32], x0: usize, y0: usize, w: usize, h: usize, level: f32) {
    let fill = (w as f32 * level.clamp(0.0, 1.0)) as usize;
    for x in 0..w {
        let col = if x >= fill { DIM } else if x as f32 / w as f32 > 0.85 { HI } else { INK };
        for y in y0..y0 + h { put(buf, x0 + x, y, col); }
    }
}

// Work-time sparkline; budget line marks the 60 Hz ceiling.
fn spark(buf: &mut [u32], hist: &VecDeque<u64>, peak_us: u64, top: usize, bot: usize) {
    let h = (bot - top) as u64;
    let scale = peak_us.max(FRAME_US * 2).max(1);
    let by = bot - ((FRAME_US * h / scale) as usize).min(h as usize);
    for x in 0..PW { put(buf, x, by, DIM); }
    let n = hist.len();
    for (i, &w) in hist.iter().enumerate() {
        let x = PW - n + i;
        let bh = (w * h / scale).min(h) as usize;
        let col = if w > FRAME_US { HI } else { INK };
        for y in (bot - bh)..=bot { put(buf, x, y, col); }
    }
}

#[inline]
fn put(buf: &mut [u32], x: usize, y: usize, c: u32) {
    if x < PW && y < PH { buf[y * PW + x] = c; }
}

fn human(n: u64) -> String {
    if n >= 1_000_000_000 { format!("{:.1}B", n as f64 / 1e9) }
    else if n >= 1_000_000 { format!("{:.1}M", n as f64 / 1e6) }
    else if n >= 1_000 { format!("{:.1}K", n as f64 / 1e3) }
    else { format!("{}", n) }
}

// 8x8 font: glyph row = FONT[(cp-32)*8 + row], bit 7-col = leftmost.
fn text(buf: &mut [u32], x0: usize, y0: usize, s: &str, scale: usize, color: u32) {
    let mut x = x0;
    for ch in s.chars() {
        let cp = ch as u32;
        if (32..127).contains(&cp) {
            let base = (cp as usize - 32) * 16;
            for row in 0..16 {
                let bits = FONT.get(base + row).copied().unwrap_or(0);
                for col in 0..8 {
                    if bits & (1 << (7 - col)) != 0 {
                        for dy in 0..scale {
                            for dx in 0..scale {
                                put(buf, x + col * scale + dx, y0 + row * scale + dy, color);
                            }
                        }
                    }
                }
            }
        }
        x += 8 * scale;
    }
}

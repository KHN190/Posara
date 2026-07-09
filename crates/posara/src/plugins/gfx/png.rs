use std::path::Path;

use super::Framebuffer;

impl Framebuffer {
    // RGB565 region → 8-bit RGB PNG. Out-of-bounds region is clamped to the
    // framebuffer rect with a stderr warning; if the clamp leaves zero area
    // returns Err. `image` crate is already a dep (used by png2abe).
    pub fn save_region_png(&self, x: i64, y: i64, w: i64, h: i64, path: &Path) -> Result<(), String> {
        if self.w == 0 || self.h == 0 {
            return Err("framebuffer not configured (call screen() first)".into());
        }
        let fb_w = self.w as i64;
        let fb_h = self.h as i64;
        let x0 = x.max(0).min(fb_w);
        let y0 = y.max(0).min(fb_h);
        let x1 = (x + w).max(0).min(fb_w);
        let y1 = (y + h).max(0).min(fb_h);
        if x1 <= x0 || y1 <= y0 {
            return Err(format!("region ({x},{y},{w},{h}) outside framebuffer {}x{}", self.w, self.h));
        }
        if x != x0 || y != y0 || w != (x1 - x0) || h != (y1 - y0) {
            eprintln!(
                "fb_save_png: region ({x},{y},{w},{h}) clamped to ({x0},{y0},{},{})",
                x1 - x0, y1 - y0
            );
        }
        let rw = (x1 - x0) as usize;
        let rh = (y1 - y0) as usize;
        let mut pixels = Vec::with_capacity(rw * rh * 3);
        for yy in 0..rh {
            let src_y = y0 as usize + yy;
            for xx in 0..rw {
                let src_x = x0 as usize + xx;
                let c = self.buf[src_y * self.w + src_x];
                let r5 = ((c >> 11) & 0x1F) as u32;
                let g6 = ((c >> 5) & 0x3F) as u32;
                let b5 = (c & 0x1F) as u32;
                pixels.push(((r5 << 3) | (r5 >> 2)) as u8);
                pixels.push(((g6 << 2) | (g6 >> 4)) as u8);
                pixels.push(((b5 << 3) | (b5 >> 2)) as u8);
            }
        }
        let img = image::RgbImage::from_raw(rw as u32, rh as u32, pixels)
            .ok_or_else(|| "RgbImage::from_raw failed".to_string())?;
        img.save(path).map_err(|e| e.to_string())
    }
}

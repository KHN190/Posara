// img2sprite — convert PNG/JPEG/... into a 1bpp sprite for posara.
//
// Output is a raw 1bpp bitmap: W*H bits, row-major, MSB-first, ceil(W*H/8)
// bytes — the exact layout `blit`/`blitg` consume. A cart loads and draws it
// just like a font glyph (rotation / XOR come from blitg at draw time):
//
//   let s = fs_read(fd, sprite_bytes);
//   blitg(s, 0, x, y, W, H, color, mode)        // mode high bits = rotation
//
// bit=1 = "ink" (drawn pixel). By default dark pixels are ink (black art on a
// white field); --invert flips. fs_read stores one byte per Array<Int> element.

use std::path::PathBuf;
use std::process::ExitCode;

use image::{imageops::FilterType, GenericImageView};

fn usage() -> ExitCode {
    eprintln!("usage: img2sprite <input.png|jpg|...> <out.spr> [opts]");
    eprintln!("  --size WxH     resize to exactly WxH (no aspect preserve)");
    eprintln!("  --max N        fit within NxN, preserve aspect");
    eprintln!("  --threshold T  luma cutoff 0..255, default 128 (ink = luma < T)");
    eprintln!("  --dither       Floyd-Steinberg to 1bit instead of hard threshold");
    eprintln!("  --invert       ink = bright instead of dark");
    eprintln!("  --colors16     4bpp palette mode: input must have <=15 colors (quantize");
    eprintln!("                 upstream, e.g. ffmpeg palettegen); --key color -> index 0");
    eprintln!("                 (transparent). Appends to existing .spr (sheet build) and");
    eprintln!("                 writes/checks <out>.pal (16 RGB888 lines, pal()-ready).");
    eprintln!("  --key RRGGBB   colors16: hex color treated as transparent (default 000000)");
    eprintln!("  --tol N        colors16: merge distance, lower keeps more colors (default 24)");
    eprintln!("  output: raw 1bpp, row-major MSB-first, ceil(W*H/8) bytes");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut pos: Vec<String> = Vec::new();
    let mut size: Option<(u32, u32)> = None;
    let mut max: Option<u32> = None;
    let mut threshold: i32 = 128;
    let mut dither = false;
    let mut invert = false;
    let mut colors16 = false;
    let mut key: u32 = 0;
    let mut tol: u32 = 24;
    let mut it = raw.into_iter();
    while let Some(a) = it.next() {
        let mut next = || it.next().ok_or_else(usage);
        match a.as_str() {
            "-h" | "--help" => return usage(),
            "--size" => {
                let v = match next() { Ok(v) => v, Err(c) => return c };
                let Some((w, h)) = v.split_once('x') else { return usage() };
                match (w.parse(), h.parse()) {
                    (Ok(w), Ok(h)) => size = Some((w, h)),
                    _ => return usage(),
                }
            }
            "--max" => {
                let v = match next() { Ok(v) => v, Err(c) => return c };
                match v.parse() { Ok(n) => max = Some(n), Err(_) => return usage() }
            }
            "--threshold" => {
                let v = match next() { Ok(v) => v, Err(c) => return c };
                match v.parse() { Ok(n) => threshold = n, Err(_) => return usage() }
            }
            "--dither" => dither = true,
            "--invert" => invert = true,
            "--colors16" => colors16 = true,
            "--key" => {
                let v = match next() { Ok(v) => v, Err(c) => return c };
                match u32::from_str_radix(&v, 16) { Ok(n) => key = n, Err(_) => return usage() }
            }
            "--tol" => {
                let v = match next() { Ok(v) => v, Err(c) => return c };
                match v.parse() { Ok(n) => tol = n, Err(_) => return usage() }
            }
            _ => pos.push(a),
        }
    }
    if pos.len() != 2 {
        return usage();
    }
    let inp = PathBuf::from(&pos[0]);
    let out = PathBuf::from(&pos[1]);

    let img = match image::open(&inp) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("open {}: {e}", inp.display());
            return ExitCode::from(1);
        }
    };
    let img = match (size, max) {
        (Some((w, h)), _) => img.resize_exact(w, h, FilterType::Lanczos3),
        (None, Some(n)) => {
            let (sw, sh) = img.dimensions();
            if sw.max(sh) > n {
                img.resize(n, n, FilterType::Lanczos3)
            } else {
                img
            }
        }
        _ => img,
    };
    let (w, h) = img.dimensions();
    if colors16 {
        return run_colors16(&img, &out, key, tol);
    }
    let luma = img.to_luma8();

    // build ink grid
    let (wu, hu) = (w as usize, h as usize);
    let mut ink = vec![false; wu * hu];
    if dither {
        let mut buf: Vec<i32> = luma.pixels().map(|p| p[0] as i32).collect();
        for y in 0..hu {
            for x in 0..wu {
                let i = y * wu + x;
                let old = buf[i];
                let on = old >= 128;
                let err = old - if on { 255 } else { 0 };
                ink[i] = !on; // dark (off) = ink, before invert
                if x + 1 < wu { buf[i + 1] += err * 7 / 16; }
                if y + 1 < hu {
                    if x > 0 { buf[i + wu - 1] += err * 3 / 16; }
                    buf[i + wu] += err * 5 / 16;
                    if x + 1 < wu { buf[i + wu + 1] += err / 16; }
                }
            }
        }
    } else {
        for y in 0..hu {
            for x in 0..wu {
                ink[y * wu + x] = (luma.get_pixel(x as u32, y as u32)[0] as i32) < threshold;
            }
        }
    }
    if invert {
        for v in ink.iter_mut() { *v = !*v; }
    }

    let sprite_bytes = (wu * hu + 7) / 8;
    let mut bytes = vec![0u8; sprite_bytes];
    for bit in 0..wu * hu {
        if ink[bit] {
            bytes[bit / 8] |= 1 << (7 - (bit & 7));
        }
    }

    if let Err(e) = std::fs::write(&out, &bytes) {
        eprintln!("write {}: {e}", out.display());
        return ExitCode::from(1);
    }
    eprintln!(
        "wrote {} ({}x{}, {} bytes; cart: blitg(spr, 0, x, y, {}, {}, color, mode))",
        out.display(), w, h, sprite_bytes, w, h
    );
    // small preview (downsample to <=48 wide)
    let pw = wu.min(48);
    let ph = (hu * pw / wu).max(1).min(24);
    for py in 0..ph {
        let row: String = (0..pw)
            .map(|px| {
                let sx = px * wu / pw;
                let sy = py * hu / ph;
                if ink[sy * wu + sx] { '#' } else { '.' }
            })
            .collect();
        eprintln!("  {row}");
    }
    ExitCode::SUCCESS
}

// 4bpp palette mode. Builds an animation sheet: pixels append to <out> (so a
// frame loop can call this once per frame), the palette lands in <out>.pal as
// 16 RGB888 integers (index 0 = transparent key) and must match across frames.
fn run_colors16(img: &image::DynamicImage, out: &PathBuf, key: u32, tol: u32) -> ExitCode {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let pal_path = out.with_extension("pal");
    // entry 0 is the transparent key and value 0 is padding; only real colors load.
    let mut pal: Vec<u32> = if pal_path.exists() {
        std::fs::read_to_string(&pal_path).unwrap_or_default()
            .lines().filter_map(|l| l.trim().parse().ok())
            .skip(1).filter(|&v: &u32| v != 0).collect()
    } else {
        Vec::new()
    };
    let mut rgb_of: Vec<u32> = vec![key];   // reconstructed source colors per index
    for &v in &pal {
        rgb_of.push(v);
    }

    let mut nibbles: Vec<u8> = Vec::with_capacity((w * h) as usize);
    for p in rgba.pixels() {
        let (r, g, b, a) = (p[0] as u32, p[1] as u32, p[2] as u32, p[3]);
        let rgb = (r << 16) | (g << 8) | b;
        if a < 128 || rgb == key {
            nibbles.push(0);
            continue;
        }
        // nearest existing entry within tolerance, else allocate
        let mut best = 0usize;
        let mut bd = u32::MAX;
        for (i, &c) in rgb_of.iter().enumerate().skip(1) {
            let dr = (c >> 16 & 255).abs_diff(r);
            let dg = (c >> 8 & 255).abs_diff(g);
            let db = (c & 255).abs_diff(b);
            let d = dr * dr + dg * dg + db * db;
            if d < bd { bd = d; best = i; }
        }
        let idx = if bd <= tol * tol * 3 {
            best
        } else {
            if rgb_of.len() >= 16 {
                eprintln!("more than 15 colors (at pixel rgb #{rgb:06x}); quantize upstream first");
                return ExitCode::from(1);
            }
            rgb_of.push(rgb);
            pal.push(rgb);
            rgb_of.len() - 1
        };
        nibbles.push(idx as u8);
    }
    if nibbles.len() % 2 == 1 { nibbles.push(0); }
    let bytes: Vec<u8> = nibbles.chunks(2).map(|c| (c[0] << 4) | c[1]).collect();
    use std::io::Write;
    let mut f = match std::fs::OpenOptions::new().create(true).append(true).open(out) {
        Ok(f) => f,
        Err(e) => { eprintln!("open {}: {e}", out.display()); return ExitCode::from(1); }
    };
    if let Err(e) = f.write_all(&bytes) {
        eprintln!("write: {e}");
        return ExitCode::from(1);
    }
    let mut lines: Vec<String> = pal.iter().map(|v| v.to_string()).collect();
    while lines.len() < 15 { lines.push("0".into()); }
    let _ = std::fs::write(&pal_path, format!("0\n{}\n", lines.join("\n")));
    eprintln!("{}x{} frame appended to {} ({} colors)", w, h, out.display(), rgb_of.len());
    ExitCode::SUCCESS
}

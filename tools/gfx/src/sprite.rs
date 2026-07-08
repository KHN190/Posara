// `posara-gfx sprite` — image -> posara sprite.
//
// 1bpp (default): W*H bits, row-major, MSB-first — the blit/blitg layout.
//   bit=1 = ink (drawn). Dark pixels ink by default; --invert flips.
// 4bpp --colors16: input must have <=15 colors (quantize upstream, e.g. ffmpeg
//   palettegen); --key -> index 0 (transparent). Appends to <out> (sheet build)
//   and writes/checks <out>.pal (16 RGB888 lines, pal()-ready).
// 4bpp --quant: auto-quantize (NeuQuant) to 16 colors, single frame, overwrite
//   <out> + <out>.pal. Index 0 is transparent to the `sprite` native.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use color_quant::NeuQuant;
use image::{imageops::FilterType, GenericImageView};

use crate::bitpack::pack_1bpp;

fn usage() -> ExitCode {
    eprintln!("usage: posara-gfx sprite <input.png|jpg|...> <out.spr> [opts]");
    eprintln!("  --size WxH     resize to exactly WxH (no aspect preserve)");
    eprintln!("  --max N        fit within NxN, preserve aspect");
    eprintln!("  --threshold T  luma cutoff 0..255, default 128 (ink = luma < T)");
    eprintln!("  --dither       Floyd-Steinberg to 1bit instead of hard threshold");
    eprintln!("  --invert       ink = bright instead of dark");
    eprintln!("  --colors16     4bpp palette mode, input <=15 colors, appends frames");
    eprintln!("  --quant        4bpp NeuQuant auto (16 colors, overwrites out + out.pal)");
    eprintln!("  --key RRGGBB   colors16: hex color treated as transparent (default 000000)");
    eprintln!("  --tol N        colors16: merge distance (default 24)");
    ExitCode::from(2)
}

pub fn run(args: Vec<String>) -> ExitCode {
    let mut pos: Vec<String> = Vec::new();
    let mut size: Option<(u32, u32)> = None;
    let mut max: Option<u32> = None;
    let mut threshold: i32 = 128;
    let mut dither = false;
    let mut invert = false;
    let mut colors16 = false;
    let mut quant = false;
    let mut key: u32 = 0;
    let mut tol: u32 = 24;
    let mut it = args.into_iter();
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
            "--quant" => quant = true,
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
    if quant {
        return run_quant(&img, &out);
    }
    if colors16 {
        return run_colors16(&img, &out, key, tol);
    }
    let luma = img.to_luma8();

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

    let bytes = pack_1bpp(&ink);
    if let Err(e) = std::fs::write(&out, &bytes) {
        eprintln!("write {}: {e}", out.display());
        return ExitCode::from(1);
    }
    eprintln!(
        "wrote {} ({}x{}, {} bytes; cart: gfx_blitg(&spr, 0, x, y, {}, {}, color, mode))",
        out.display(), w, h, bytes.len(), w, h
    );
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
    let mut pal: Vec<u32> = if pal_path.exists() {
        std::fs::read_to_string(&pal_path).unwrap_or_default()
            .lines().filter_map(|l| l.trim().parse().ok())
            .skip(1).filter(|&v: &u32| v != 0).collect()
    } else {
        Vec::new()
    };
    let mut rgb_of: Vec<u32> = vec![key];
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

// 4bpp auto-quantize (NeuQuant). Single frame, overwrites <out> + <out>.pal.
// index 0 = transparent to the `sprite` native, so the palette's first slot is
// reserved and real colors occupy 1..15.
fn run_quant(img: &image::DynamicImage, out: &PathBuf) -> ExitCode {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let n_px = (w * h) as usize;
    // quantize to 15 colors, shifted into indices 1..15 (0 stays transparent).
    let nq = NeuQuant::new(10, 15, rgba.as_raw());
    let pal_rgba = nq.color_map_rgba();
    let mut pal_888 = [0u32; 16];
    for i in 0..15 {
        let r = pal_rgba[i * 4] as u32;
        let g = pal_rgba[i * 4 + 1] as u32;
        let b = pal_rgba[i * 4 + 2] as u32;
        pal_888[i + 1] = (r << 16) | (g << 8) | b;
    }
    let mut nibbles: Vec<u8> = Vec::with_capacity(n_px);
    for i in 0..n_px {
        let p = &rgba.as_raw()[i * 4..i * 4 + 4];
        if p[3] < 128 {
            nibbles.push(0);
        } else {
            nibbles.push((nq.index_of(p) as u8 & 0xF) + 1);
        }
    }
    if nibbles.len() % 2 == 1 { nibbles.push(0); }
    let bytes: Vec<u8> = nibbles.chunks(2).map(|c| (c[0] << 4) | c[1]).collect();
    if let Err(e) = std::fs::write(out, &bytes) {
        eprintln!("write {}: {e}", out.display());
        return ExitCode::from(1);
    }
    let pal_path = out.with_extension("pal");
    let lines: Vec<String> = pal_888[1..].iter().map(|v| v.to_string()).collect();
    let _ = std::fs::write(&pal_path, format!("0\n{}\n", lines.join("\n")));
    eprintln!(
        "wrote {} ({}x{}, {} bytes, 15 colors +transparent; cart: gfx_sprite(&spr, 0, x, y, {}, {}, 100, 256))",
        out.display(), w, h, bytes.len(), w, h
    );
    ExitCode::SUCCESS
}

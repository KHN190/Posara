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

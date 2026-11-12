// `posara-gfx icon` - rasterize a SVG into a 1bpp bitmap-font atlas.
//
// Drawn the way pixel icons are drawn by hand: the ink box is scaled to fill the cell minus a
// margin, the stroke is a whole number of device pixels, and the shape is shifted by the
// sub-pixel offset that lands most of it on whole pixels; only coverage that cannot be aligned
// becomes gray. Output: two 1bpp planes (white, then gray), N*N bits each, row-major MSB-first —
// draw the gray plane, then the white plane, with gfx_blitg.

use std::path::PathBuf;
use std::process::ExitCode;

use resvg::{tiny_skia, usvg};

use crate::bitpack::pack_1bpp;

const SS: u32 = 8; // supersampling per device pixel
const OFFSETS: i32 = 4; // sub-pixel offsets tried per axis: -1/2, -1/4, 0, 1/4 px

fn usage() -> ExitCode {
    eprintln!(
        "usage: posara-gfx icon <input.svg|input.txt> <out.icn> [--size N] [--stroke PX] [--pad PX]"
    );
    eprintln!("  --size N     icon edge in device pixels (default 12; 8, 16, 24 ...)");
    eprintln!("  --stroke PX  stroke width in device pixels (default 1)");
    eprintln!("  --pad PX     margin; the ink is scaled to fill N - 2*PAD (default 1)");
    eprintln!("  input.txt: hand-drawn master, N rows of N chars, '#' white, '+' gray, '.' black;");
    eprintln!("      --size resamples it to any size, keeping its mirror symmetry");
    eprintln!("  writes <out> = white plane ++ gray plane (N*N bits each)");
    ExitCode::from(2)
}

// Maps source units to device pixels: device = (source + shift) * scale.
struct Fit {
    scale: f32,
    shift: (f32, f32),
}

// Coverage 0..=255 per device pixel for one sub-pixel placement.
fn coverage(tree: &usvg::Tree, n: u32, fit: &Fit, dx: f32, dy: f32) -> Vec<u8> {
    let px = n * SS;
    let mut map = tiny_skia::Pixmap::new(px, px).unwrap();
    let s = fit.scale * SS as f32;
    let ts = tiny_skia::Transform::from_translate(fit.shift.0, fit.shift.1)
        .post_scale(s, s)
        .post_translate(dx * SS as f32, dy * SS as f32);
    resvg::render(tree, ts, &mut map.as_mut());
    let a = map.data();
    (0..n * n)
        .map(|i| {
            let (cx, cy) = (i % n, i / n);
            let mut sum = 0u32;
            for y in 0..SS {
                for x in 0..SS {
                    sum += a[(((cy * SS + y) * px + cx * SS + x) * 4 + 3) as usize] as u32;
                }
            }
            (sum / (SS * SS)) as u8
        })
        .collect()
}

// 0 black, 1 gray, 2 white.
fn tone(c: u8) -> u8 {
    if c >= 160 {
        2
    } else if c >= 48 {
        1
    } else {
        0
    }
}

// Distance between the ink centroid and the cell centre, in gray-pixel units (white weighs 2).
fn off_centre(tones: &[u8], n: u32) -> f32 {
    let (mut sx, mut sy, mut m) = (0.0, 0.0, 0.0);
    for (i, &t) in tones.iter().enumerate() {
        let w = t as f32;
        sx += w * (i as u32 % n) as f32;
        sy += w * (i as u32 / n) as f32;
        m += w;
    }
    if m == 0.0 {
        return 0.0;
    }
    let c = (n as f32 - 1.0) / 2.0;
    ((sx / m - c).powi(2) + (sy / m - c).powi(2)).sqrt() * 4.0
}

fn parse(src: &str, path: &PathBuf) -> Result<usvg::Tree, ExitCode> {
    usvg::Tree::from_str(src, &usvg::Options::default()).map_err(|e| {
        eprintln!("parse {}: {e}", path.display());
        ExitCode::from(1)
    })
}

pub fn run(args: Vec<String>) -> ExitCode {
    let mut pos = Vec::new();
    let (mut n, mut stroke, mut pad, mut sized) = (12u32, 1.0f32, 1.0f32, false);
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        let value = match a.as_str() {
            "--size" | "--stroke" | "--pad" => {
                match it.next().and_then(|v| v.parse::<f32>().ok()) {
                    Some(v) => v,
                    None => return usage(),
                }
            }
            "-h" | "--help" => return usage(),
            _ => {
                pos.push(a);
                continue;
            }
        };
        match a.as_str() {
            "--size" => {
                n = value as u32;
                sized = true
            }
            "--stroke" => stroke = value,
            _ => pad = value,
        }
    }
    if pos.len() != 2 || n == 0 {
        return usage();
    }
    let (inp, out) = (PathBuf::from(&pos[0]), PathBuf::from(&pos[1]));
    // Hand-drawn pixels: one row per line, '#' white, '+' gray, '.' black; the file sets the size.
    if inp.extension().is_some_and(|e| e == "txt") {
        let text = match std::fs::read_to_string(&inp) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("open {}: {e}", inp.display());
                return ExitCode::from(1);
            }
        };
        let rows: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
        if rows.iter().any(|r| r.len() != rows.len()) {
            eprintln!("{}: expected a square grid", inp.display());
            return ExitCode::from(1);
        }
        let tones: Vec<u8> = rows
            .iter()
            .flat_map(|r| r.bytes())
            .map(|b| match b {
                b'#' => 2,
                b'+' => 1,
                _ => 0,
            })
            .collect();
        let m = rows.len() as u32;
        let size = if sized { n } else { m };
        return write(&out, &resample(&tones, m, size), size);
    }
    let src = match std::fs::read_to_string(&inp) {
        Ok(s) => s.replace("currentColor", "#fff"),
        Err(e) => {
            eprintln!("open {}: {e}", inp.display());
            return ExitCode::from(1);
        }
    };
    // The ink box without stroke decides scale, so every icon fills the same cell.
    let shape = match parse(
        &src.replace("stroke-width=\"2\"", "stroke-width=\"0.001\""),
        &inp,
    ) {
        Ok(t) => t,
        Err(c) => return c,
    };
    let b = shape.root().abs_bounding_box();
    let room = (n as f32 - 2.0 * pad - stroke).max(1.0);
    let scale = room / b.width().max(b.height()).max(1e-3);
    let half = n as f32 / 2.0 / scale;
    let fit = Fit {
        scale,
        shift: (
            half - b.x() - b.width() / 2.0,
            half - b.y() - b.height() / 2.0,
        ),
    };
    let tree = match parse(
        &src.replace(
            "stroke-width=\"2\"",
            &format!("stroke-width=\"{}\"", stroke / scale),
        ),
        &inp,
    ) {
        Ok(t) => t,
        Err(c) => return c,
    };
    // Pick the placement with the fewest gray pixels, then the one whose ink sits closest to centre.
    let mut best: Option<(f32, Vec<u8>)> = None;
    for oy in 0..OFFSETS {
        for ox in 0..OFFSETS {
            let (dx, dy) = (
                ox as f32 / OFFSETS as f32 - 0.5,
                oy as f32 / OFFSETS as f32 - 0.5,
            );
            let tones: Vec<u8> = coverage(&tree, n, &fit, dx, dy)
                .into_iter()
                .map(tone)
                .collect();
            let score = tones.iter().filter(|&&t| t == 1).count() as f32 + off_centre(&tones, n);
            if best.as_ref().is_none_or(|(g, _)| score < *g) {
                best = Some((score, tones));
            }
        }
    }
    let (_, tones) = best.unwrap();
    write(&out, &tones, n)
}

fn write(out: &PathBuf, tones: &[u8], n: u32) -> ExitCode {
    let mut bytes = pack_1bpp(&tones.iter().map(|&t| t == 2).collect::<Vec<_>>());
    bytes.extend(pack_1bpp(
        &tones.iter().map(|&t| t == 1).collect::<Vec<_>>(),
    ));
    if let Err(e) = std::fs::write(&out, &bytes) {
        eprintln!("write {}: {e}", out.display());
        return ExitCode::from(1);
    }
    for y in 0..n as usize {
        let row: String = tones[y * n as usize..(y + 1) * n as usize]
            .iter()
            .map(|t| [' ', '+', '#'][*t as usize])
            .collect();
        eprintln!("|{row}|");
    }
    eprintln!(
        "wrote {} ({n}x{n}, white plane then gray plane, {} bytes)",
        out.display(),
        bytes.len()
    );
    ExitCode::SUCCESS
}

// Resamples an m×m pixel master to n×n by area
fn resample(src: &[u8], m: u32, n: u32) -> Vec<u8> {
    if m == n {
        return src.to_vec();
    }
    let at = |x: u32, y: u32| src[(y * m + x) as usize];
    let mirror_x = (0..m).all(|y| (0..m).all(|x| at(x, y) == at(m - 1 - x, y)));
    let mirror_y = (0..m).all(|y| (0..m).all(|x| at(x, y) == at(x, m - 1 - y)));
    let fold = |i: u32, on: bool| {
        if on && i >= n.div_ceil(2) {
            n - 1 - i
        } else {
            i
        }
    };
    let span = |i: u32, j: u32| {
        let (a, b) = (
            i as f32 * m as f32 / n as f32,
            (i + 1) as f32 * m as f32 / n as f32,
        );
        (b.min((j + 1) as f32) - a.max(j as f32)).max(0.0)
    };
    let cell = |tx: u32, ty: u32| {
        let mut area = [0.0f32; 3];
        for y in 0..m {
            for x in 0..m {
                area[at(x, y) as usize] += span(tx, x) * span(ty, y);
            }
        }
        let total: f32 = area.iter().sum();
        if area[0] >= total / 3.0 && area[0] < total {
            return 0;
        }
        (0..3).max_by(|&a, &b| area[a].total_cmp(&area[b])).unwrap() as u8
    };
    (0..n * n)
        .map(|i| cell(fold(i % n, mirror_x), fold(i / n, mirror_y)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::resample;

    #[test]
    fn resample_keeps_size_and_symmetry() {
        let plus = [0, 2, 0, 2, 2, 2, 0, 2, 0];
        for n in 2..=12u32 {
            let out = resample(&plus, 3, n);
            assert_eq!(out.len(), (n * n) as usize);
            for y in 0..n {
                for x in 0..n {
                    let v = out[(y * n + x) as usize];
                    assert_eq!(v, out[(y * n + n - 1 - x) as usize], "n={n} x mirror");
                    assert_eq!(v, out[((n - 1 - y) * n + x) as usize], "n={n} y mirror");
                }
            }
        }
        assert_eq!(resample(&plus, 3, 3), plus);
    }
}

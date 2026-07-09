// `posara-gfx font` — rasterize a TTF/OTF into a 1bpp bitmap-font atlas.
//
// Output: `count` glyphs from codepoint `first`, each a WxH 1bpp bitmap,
// row-major MSB-first, ceil(W*H/8) bytes — the layout `blit` consumes. Load
// glyph c with fs_seek(fd, (c-first)*glyph_bytes) then gfx_blitg.

use std::path::PathBuf;
use std::process::ExitCode;

use crate::bitpack::pack_1bpp;

struct Args {
    font: PathBuf,
    out: PathBuf,
    gw: usize,
    gh: usize,
    px: f32,
    first: u32,
    count: u32,
    baseline: i32,
}

fn usage() -> ExitCode {
    eprintln!("usage: posara-gfx font <font.ttf|otf> <out.fnt> [opts]");
    eprintln!("  --cell WxH     glyph cell, default 8x8");
    eprintln!("  --px N         raster pixel height, default = cell H");
    eprintln!("  --first C      first codepoint, default 32 (space)");
    eprintln!("  --count N      glyph count, default 95 (ASCII 32..126)");
    eprintln!("  --baseline B   baseline row in cell, default H-2");
    ExitCode::from(2)
}

fn parse(args: Vec<String>) -> Result<Args, ExitCode> {
    let mut pos: Vec<String> = Vec::new();
    let (mut gw, mut gh) = (8usize, 8usize);
    let mut px: Option<f32> = None;
    let mut first = 32u32;
    let mut count = 95u32;
    let mut baseline: Option<i32> = None;
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        let mut next = || it.next().ok_or_else(usage);
        match a.as_str() {
            "-h" | "--help" => return Err(usage()),
            "--cell" => {
                let v = next()?;
                let (w, h) = v.split_once('x').ok_or_else(usage)?;
                gw = w.parse().map_err(|_| usage())?;
                gh = h.parse().map_err(|_| usage())?;
            }
            "--px" => px = Some(next()?.parse().map_err(|_| usage())?),
            "--first" => first = next()?.parse().map_err(|_| usage())?,
            "--count" => count = next()?.parse().map_err(|_| usage())?,
            "--baseline" => baseline = Some(next()?.parse().map_err(|_| usage())?),
            _ => pos.push(a),
        }
    }
    if pos.len() != 2 {
        return Err(usage());
    }
    if gw == 0 || gh == 0 || gw > 64 || gh > 64 {
        eprintln!("cell must be 1..64 per side");
        return Err(ExitCode::from(2));
    }
    Ok(Args {
        font: PathBuf::from(&pos[0]),
        out: PathBuf::from(&pos[1]),
        gw,
        gh,
        px: px.unwrap_or(gh as f32),
        first,
        count,
        baseline: baseline.unwrap_or(gh as i32 - 2),
    })
}

pub fn run(argv: Vec<String>) -> ExitCode {
    let args = match parse(argv) {
        Ok(a) => a,
        Err(code) => return code,
    };

    let data = match std::fs::read(&args.font) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("read {}: {e}", args.font.display());
            return ExitCode::from(1);
        }
    };
    let font = match fontdue::Font::from_bytes(data, fontdue::FontSettings::default()) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("parse font: {e}");
            return ExitCode::from(1);
        }
    };

    let (gw, gh) = (args.gw, args.gh);
    let glyph_bytes = (gw * gh + 7) / 8;
    let mut out = Vec::with_capacity(glyph_bytes * args.count as usize);
    let mut preview: Option<(u32, Vec<bool>)> = None;

    for i in 0..args.count {
        let cp = args.first + i;
        let ch = char::from_u32(cp).unwrap_or(' ');
        let (m, bm) = font.rasterize(ch, args.px);

        let mut cell = vec![false; gw * gh];
        let left = m.xmin;
        let top = args.baseline - m.ymin - m.height as i32;
        for by in 0..m.height {
            for bx in 0..m.width {
                if bm[by * m.width + bx] > 127 {
                    let cx = left + bx as i32;
                    let cy = top + by as i32;
                    if cx >= 0 && cx < gw as i32 && cy >= 0 && cy < gh as i32 {
                        cell[(cy * gw as i32 + cx) as usize] = true;
                    }
                }
            }
        }

        out.extend_from_slice(&pack_1bpp(&cell));

        if ch == 'A' {
            preview = Some((cp, cell));
        }
    }

    if let Err(e) = std::fs::write(&args.out, &out) {
        eprintln!("write {}: {e}", args.out.display());
        return ExitCode::from(1);
    }

    eprintln!(
        "wrote {} ({} glyphs, cell {}x{}, {} bytes/glyph, first cp {}, {} bytes total)",
        args.out.display(), args.count, gw, gh, glyph_bytes, args.first, out.len()
    );
    if let Some((cp, cell)) = preview {
        eprintln!("preview cp {} ('A'):", cp);
        for y in 0..gh {
            let row: String = (0..gw)
                .map(|x| if cell[y * gw + x] { '#' } else { '.' })
                .collect();
            eprintln!("  {row}");
        }
    }
    ExitCode::SUCCESS
}

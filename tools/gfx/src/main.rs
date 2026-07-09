// posara-gfx — offline image asset converter.
//   posara-gfx sprite <in.png> <out.spr> [opts]   image -> 1bpp/4bpp sprite
//   posara-gfx font   <in.ttf> <out.fnt> [opts]   font  -> 1bpp glyph atlas

use std::process::ExitCode;

mod bitpack;
mod font;
mod sprite;

fn usage() -> ExitCode {
    eprintln!("usage: posara-gfx <sprite|font> ...");
    eprintln!("  sprite   image -> .spr (run with -h for opts)");
    eprintln!("  font     ttf/otf -> .fnt");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("sprite") => sprite::run(args.collect()),
        Some("font") => font::run(args.collect()),
        _ => usage(),
    }
}

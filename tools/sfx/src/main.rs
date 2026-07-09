// posara-sfx — offline audio asset converter.
//   posara-sfx sample <in.mp3> <out.sample> [--rate N]   mp3 -> 1-bit stream
//   posara-sfx track  <in.mid> <out.trk> [opts]          midi -> packed events

use std::process::ExitCode;

mod sample;
mod track;

fn usage() -> ExitCode {
    eprintln!("usage: posara-sfx <sample|track> ...");
    eprintln!("  sample   mp3 -> 1-bit delta-sigma .sample (run with -h for opts)");
    eprintln!("  track    midi -> .trk packed events");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("sample") => sample::run(args.collect()),
        Some("track") => track::run(args.collect()),
        _ => usage(),
    }
}

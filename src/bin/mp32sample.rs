// mp32sample — mp3 → 1-bit delta-sigma stream
//
//   decode mp3 → mono → linear resample → delta-sigma → packed bits
//
// output raw bit stream (MSB-first) cart:
//   let s = fs_read(fd, BYTES); sfx_sample(s, 8000, vol)

use std::path::PathBuf;
use std::process::ExitCode;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

fn usage() -> ExitCode {
    eprintln!("usage: mp32sample <in.mp3> <out.1bit> [--rate N]");
    eprintln!("  --rate N   delta-sigma sample rate (default 8000; = sfx_sample's rate arg)");
    eprintln!("  output: raw bit stream, MSB-first, ceil(samples/8) bytes");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut pos: Vec<String> = Vec::new();
    let mut rate = 8000u32;
    let mut it = raw.into_iter();
    while let Some(a) = it.next() {
        let mut next = || it.next().ok_or_else(usage);
        match a.as_str() {
            "-h" | "--help" => return usage(),
            "--rate" => match next() { Ok(v) => match v.parse() { Ok(n) => rate = n, _ => return usage() }, Err(c) => return c },
            _ => pos.push(a),
        }
    }
    if pos.len() != 2 { return usage(); }
    let inp = PathBuf::from(&pos[0]);
    let out = PathBuf::from(&pos[1]);

    let (mono, sr) = match decode_mono(&inp) {
        Ok(v) => v,
        Err(e) => { eprintln!("decode {}: {e}", inp.display()); return ExitCode::from(1); }
    };
    if mono.is_empty() { eprintln!("no audio decoded"); return ExitCode::from(1); }

    let resampled = if sr == rate { mono } else { resample(&mono, sr, rate) };

    // 1st-order delta-sigma: error-feedback quantize to ±1.
    let mut err = 0.0f32;
    let n = resampled.len();
    let mut bytes = vec![0u8; (n + 7) / 8];
    for (i, &x) in resampled.iter().enumerate() {
        let v = x.clamp(-0.95, 0.95) + err;
        let bit = v >= 0.0;
        err = v - if bit { 1.0 } else { -1.0 };
        if bit { bytes[i / 8] |= 1 << (7 - (i & 7)); }
    }

    if let Err(e) = std::fs::write(&out, &bytes) {
        eprintln!("write {}: {e}", out.display());
        return ExitCode::from(1);
    }
    eprintln!(
        "wrote {} ({} samples @ {}Hz, {} bytes; cart: let s = fs_read(fd, {}); sfx_sample(s, {}, 80))",
        out.display(), n, rate, bytes.len(), bytes.len(), rate
    );
    ExitCode::SUCCESS
}

// Linear interpolation resample. Output is 1-bit delta-sigma'd anyway, so
// rubato's sinc filtering buys nothing here — saves rustfft + transitive deps.
fn resample(input: &[f32], src: u32, dst: u32) -> Vec<f32> {
    if input.is_empty() { return Vec::new(); }
    let n_out = ((input.len() as u64) * (dst as u64) / (src as u64).max(1)) as usize;
    let ratio = src as f64 / dst as f64;
    let mut out = Vec::with_capacity(n_out);
    let last = input.len() - 1;
    for i in 0..n_out {
        let pos = i as f64 * ratio;
        let i0 = (pos as usize).min(last);
        let i1 = (i0 + 1).min(last);
        let t = (pos - i0 as f64) as f32;
        out.push(input[i0] + (input[i1] - input[i0]) * t);
    }
    out
}

fn decode_mono(path: &PathBuf) -> Result<(Vec<f32>, u32), String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp3");
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| e.to_string())?;
    let mut format = probed.format;
    let track = format.default_track().ok_or("no default track")?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| e.to_string())?;

    let mut mono: Vec<f32> = Vec::new();
    let mut sr = 0u32;
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(_) => break,
        };
        if packet.track_id() != track_id { continue; }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let spec = *decoded.spec();
        sr = spec.rate;
        let ch = spec.channels.count().max(1);
        let mut buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        buf.copy_interleaved_ref(decoded);
        for frame in buf.samples().chunks(ch) {
            mono.push(frame.iter().sum::<f32>() / ch as f32);
        }
    }
    if sr == 0 { return Err("no samples".into()); }
    Ok((mono, sr))
}

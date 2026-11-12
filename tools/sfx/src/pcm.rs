use std::path::PathBuf;
use std::process::ExitCode;
fn convert(input: &[f32], src: u32, dst: u32) -> Vec<f32> {
    let count = (input.len() as u64 * dst as u64 / src as u64) as usize;
    let cutoff = (dst as f64 / src as f64).min(1.0) * 0.94;
    (0..count)
        .map(|i| {
            let p = i as f64 * src as f64 / dst as f64;
            let center = p.floor() as isize;
            let (mut sum, mut weight) = (0.0, 0.0);
            for offset in -32..=32 {
                let index = center + offset;
                let d = index as f64 - p;
                let x = std::f64::consts::PI * d * cutoff;
                let sinc = if x.abs() < 1e-10 { 1.0 } else { x.sin() / x };
                let window = 0.5 + 0.5 * (std::f64::consts::PI * d / 33.0).cos();
                let w = sinc * window * cutoff;
                if index >= 0 && (index as usize) < input.len() {
                    sum += input[index as usize] as f64 * w;
                }
                weight += w;
            }
            (sum / weight) as f32
        })
        .collect()
}
pub fn run(args: Vec<String>) -> ExitCode {
    let result = (|| -> Result<(), String> {
        if args.len() != 2 && args.len() != 4 {
            return Err("usage: posara-sfx pcm INPUT OUTPUT.pcm [--rate 24000]".into());
        }
        let rate = if args.len() == 4 {
            if args[2] != "--rate" {
                return Err("expected --rate".into());
            }
            args[3].parse::<u32>().map_err(|_| "invalid rate")?
        } else {
            24000
        };
        if !(8000..=96000).contains(&rate) {
            return Err("rate must be 8000..96000".into());
        }
        if PathBuf::from(&args[0]).canonicalize().ok()
            == PathBuf::from(&args[1]).canonicalize().ok()
            && PathBuf::from(&args[1]).exists()
        {
            return Err("input and output must differ".into());
        }
        let (input, src) = super::sample::decode_mono(&PathBuf::from(&args[0]))?;
        if input.is_empty() || input.iter().any(|x| !x.is_finite()) {
            return Err("empty or non-finite audio".into());
        }
        let mut samples = if src == rate {
            input
        } else {
            convert(&input, src, rate)
        };
        // Remove the recording's DC offset; preserve relative level across a kit.
        let mean = samples.iter().map(|x| *x as f64).sum::<f64>() / samples.len() as f64;
        for x in &mut samples {
            *x -= mean as f32;
        }
        let fade = (rate as usize / 1000).min(samples.len() / 2);
        let len = samples.len();
        for i in 0..fade {
            let g = i as f32 / fade as f32;
            samples[i] *= g;
            samples[len - 1 - i] *= g;
        }
        let count = u32::try_from(samples.len()).map_err(|_| "audio too long")?;
        let mut bytes = Vec::with_capacity(12 + samples.len() * 2);
        bytes.extend_from_slice(b"PSM1");
        bytes.extend_from_slice(&rate.to_le_bytes());
        bytes.extend_from_slice(&count.to_le_bytes());
        for x in samples {
            bytes.extend_from_slice(&((x.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes());
        }
        std::fs::write(&args[1], &bytes).map_err(|e| e.to_string())?;
        eprintln!(
            "{}: {count} frames, {rate} Hz, {} bytes",
            args[1],
            bytes.len()
        );
        Ok(())
    })();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resampling_preserves_passband_and_rejects_aliases() {
        for src in [44100, 48000] {
            for dst in [16000, 24000] {
                for frequency in [440.0, 1000.0, 15000.0] {
                    let input: Vec<_> = (0..src / 10)
                        .map(|i| {
                            (std::f32::consts::TAU * frequency * i as f32 / src as f32).sin() * 0.5
                        })
                        .collect();
                    let out = convert(&input, src, dst);
                    assert_eq!(
                        out.len(),
                        (input.len() as u64 * dst as u64 / src as u64) as usize
                    );
                    let rms = (out[100..out.len() - 100].iter().map(|x| x * x).sum::<f32>()
                        / (out.len() - 200) as f32)
                        .sqrt();
                    if frequency < 2000.0 {
                        assert!((rms - 0.35355).abs() < 0.01);
                    } else {
                        assert!(rms < 0.01, "{src} {dst} {rms}");
                    }
                }
            }
        }
    }
}

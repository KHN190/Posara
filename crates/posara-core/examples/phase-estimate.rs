fn approx(p: f32, n: usize) -> f32 {
    let q = (p * 4.0 + 0.5) as u32;
    let x = (p - q as f32 * 0.25) * std::f32::consts::TAU;
    let z = x * x;
    let sin = [
        1.0f32,
        -1.0 / 6.0,
        1.0 / 120.0,
        -1.0 / 5040.0,
        1.0 / 362880.0,
    ];
    let cos = [1.0f32, -0.5, 1.0 / 24.0, -1.0 / 720.0, 1.0 / 40320.0];
    let c = if q & 1 == 0 { sin } else { cos };
    let mut v = c[n];
    for k in (0..n).rev() {
        v = c[k] + z * v;
    }
    if q & 1 == 0 {
        v *= x;
    }
    if q & 2 == 0 { v } else { -v }
}
fn main() {
    println!("sin_degree,cos_degree,max_error,rms_error,worst_join_jump,fm_max_error,fm_rms_error");
    for n in (1..=4).rev() {
        let (mut max, mut sq) = (0.0f64, 0.0f64);
        for i in 0..=1_048_576 {
            let p = i as f32 / 1_048_576.0;
            let e = approx(p, n) as f64 - (p as f64 * std::f64::consts::TAU).sin();
            max = max.max(e.abs());
            sq += e * e;
        }
        let mut jump = 0.0f64;
        for p in [0.125f32, 0.375, 0.625, 0.875] {
            let l = f32::from_bits(p.to_bits() - 1);
            let r = f32::from_bits(p.to_bits() + 1);
            jump = jump.max((approx(l, n) - approx(r, n)).abs() as f64);
        }
        let (mut fmmax, mut fmsq, mut count) = (0.0f64, 0.0f64, 0u64);
        for note in 0..=127 {
            let step = 440.0f32 * 2f32.powf((note as f32 - 69.0) / 12.0) / 48000.0;
            for depth in [0.0f32, 0.1, 0.5, 0.9, 1.0] {
                for ratio in [0.5f32, 1.0, 2.0, 4.0] {
                    let (mut p, mut m) = (0.0f32, 0.0f32);
                    for _ in 0..4096 {
                        p = (p + step).fract();
                        m = (m + step * ratio).fract();
                        let a = (p + approx(m, n) * depth * std::f32::consts::FRAC_1_PI)
                            .rem_euclid(1.0);
                        let b = (p as f64
                            + (m as f64 * std::f64::consts::TAU).sin() * depth as f64
                                / std::f64::consts::PI)
                            .rem_euclid(1.0);
                        let e = approx(a, n) as f64 - (b * std::f64::consts::TAU).sin();
                        fmmax = fmmax.max(e.abs());
                        fmsq += e * e;
                        count += 1;
                    }
                }
            }
        }
        println!(
            "{},{},{:.9},{:.9},{:.9},{:.9},{:.9}",
            2 * n + 1,
            2 * n,
            max,
            (sq / 1_048_577.0).sqrt(),
            jump,
            fmmax,
            (fmsq / count as f64).sqrt()
        );
    }
}

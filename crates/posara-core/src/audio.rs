pub fn to_i16(s: f32) -> i16 {
    (s.clamp(-1.0, 1.0) * 32767.0) as i16
}

pub fn to_mono_i16(stereo: &[f32], out: &mut [i16]) {
    for (o, lr) in out.iter_mut().zip(stereo.chunks_exact(2)) {
        *o = to_i16((lr[0] + lr[1]) * 0.5);
    }
}

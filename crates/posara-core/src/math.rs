#[cfg(any(feature = "gfx", feature = "sfx"))]
pub fn sin(x: f32) -> f32 { libm::sinf(x) }
#[cfg(any(feature = "gfx", feature = "sfx"))]
pub fn cos(x: f32) -> f32 { libm::cosf(x) }
#[cfg(any(feature = "gfx", feature = "sfx"))]
pub fn exp(x: f32) -> f32 { libm::expf(x) }
#[cfg(any(feature = "gfx", feature = "sfx"))]
pub fn tanh(x: f32) -> f32 { libm::tanhf(x) }
#[cfg(any(feature = "gfx", feature = "sfx"))]
pub fn powf(x: f32, n: f32) -> f32 { libm::powf(x, n) }

pub fn isqrt(n: i64) -> i64 {
    if n < 2 { return n.max(0); }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

pub fn isqrt_ceil(n: i64) -> i64 {
    let s = isqrt(n);
    if s * s < n { s + 1 } else { s }
}

#[cfg(not(feature = "std"))]
pub trait F32Ext {
    fn round(self) -> f32;
    fn sqrt(self) -> f32;
    fn ceil(self) -> f32;
    fn rem_euclid(self, rhs: f32) -> f32;
}

#[cfg(not(feature = "std"))]
impl F32Ext for f32 {
    fn round(self) -> f32 { libm::roundf(self) }
    fn sqrt(self) -> f32 { libm::sqrtf(self) }
    fn ceil(self) -> f32 { libm::ceilf(self) }
    fn rem_euclid(self, rhs: f32) -> f32 {
        let r = self % rhs;
        if r < 0.0 { r + rhs.abs() } else { r }
    }
}


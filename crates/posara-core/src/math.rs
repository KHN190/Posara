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

#[cfg(any(feature = "gfx", feature = "sfx"))]
pub trait Real: Copy + core::ops::Add<Output = Self> + core::ops::Mul<Output = Self> + core::ops::Div<Output = Self> + core::ops::Neg<Output = Self> {
    const PI: Self;
    fn of(n: i64) -> Self;
    fn int(self) -> i64;
    fn round(self) -> Self;
    fn sqrt(self) -> Self;
    fn ceil(self) -> Self;
    fn sin_cos(self) -> (Self, Self);
}

#[cfg(any(feature = "gfx", feature = "sfx"))]
impl Real for f32 {
    const PI: f32 = core::f32::consts::PI;
    fn of(n: i64) -> f32 { n as f32 }
    fn int(self) -> i64 { self as i64 }
    fn round(self) -> f32 { libm::roundf(self) }
    fn sqrt(self) -> f32 { libm::sqrtf(self) }
    fn ceil(self) -> f32 { libm::ceilf(self) }
    fn sin_cos(self) -> (f32, f32) { libm::sincosf(self) }
}

#[cfg(any(feature = "gfx", feature = "sfx"))]
impl Real for f64 {
    const PI: f64 = core::f64::consts::PI;
    fn of(n: i64) -> f64 { n as f64 }
    fn int(self) -> i64 { self as i64 }
    fn round(self) -> f64 { libm::round(self) }
    fn sqrt(self) -> f64 { libm::sqrt(self) }
    fn ceil(self) -> f64 { libm::ceil(self) }
    fn sin_cos(self) -> (f64, f64) { libm::sincos(self) }
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


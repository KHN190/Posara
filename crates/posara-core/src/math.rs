#[cfg(not(feature = "std"))]
pub trait F32Ext {
    fn sin(self) -> f32;
    fn cos(self) -> f32;
    fn exp(self) -> f32;
    fn tanh(self) -> f32;
    fn powf(self, n: f32) -> f32;
    fn round(self) -> f32;
    fn sqrt(self) -> f32;
    fn ceil(self) -> f32;
    fn sin_cos(self) -> (f32, f32);
    fn rem_euclid(self, rhs: f32) -> f32;
}

#[cfg(not(feature = "std"))]
impl F32Ext for f32 {
    fn sin(self) -> f32 { libm::sinf(self) }
    fn cos(self) -> f32 { libm::cosf(self) }
    fn exp(self) -> f32 { libm::expf(self) }
    fn tanh(self) -> f32 { libm::tanhf(self) }
    fn powf(self, n: f32) -> f32 { libm::powf(self, n) }
    fn round(self) -> f32 { libm::roundf(self) }
    fn sqrt(self) -> f32 { libm::sqrtf(self) }
    fn ceil(self) -> f32 { libm::ceilf(self) }
    fn sin_cos(self) -> (f32, f32) { (libm::sinf(self), libm::cosf(self)) }
    fn rem_euclid(self, rhs: f32) -> f32 {
        let r = self % rhs;
        if r < 0.0 { r + rhs.abs() } else { r }
    }
}


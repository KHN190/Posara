pub fn sin(x: f32) -> f32 { myriad::word32::fmath::sin(x) }
pub fn cos(x: f32) -> f32 { myriad::word32::fmath::cos(x) }
#[cfg(any(feature = "gfx", feature = "sfx"))]
pub fn exp(x: f32) -> f32 { libm::expf(x) }
#[cfg(any(feature = "gfx", feature = "sfx"))]
pub fn tanh(x: f32) -> f32 { libm::tanhf(x) }
#[cfg(any(feature = "gfx", feature = "sfx"))]
pub fn powf(x: f32, n: f32) -> f32 { pow32::powf(x, n) }
mod pow32;

pub trait IntegerSqrt: Copy { fn floor_sqrt(self) -> Self; fn ceil_sqrt(self) -> Self; }
macro_rules! integer_sqrt {
    ($ty:ty) => {
        impl IntegerSqrt for $ty {
            fn floor_sqrt(self) -> Self {
                if self < 2 { return self.max(0); }
                let mut x = self;
                let mut y = x / 2 + x % 2;
                while y < x { x = y; y = (x + self / x) / 2; }
                x
            }
            fn ceil_sqrt(self) -> Self {
                let s = self.floor_sqrt();
                if s * s < self { s + 1 } else { s }
            }
        }
    };
}
integer_sqrt!(i32);
#[cfg(not(all(target_os = "none", target_pointer_width = "32")))]
integer_sqrt!(i64);
pub fn isqrt<T: IntegerSqrt>(n: T) -> T { n.floor_sqrt() }
pub fn isqrt_ceil<T: IntegerSqrt>(n: T) -> T { n.ceil_sqrt() }

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
    fn sqrt(self) -> f32 { myriad::word32::fmath::sqrt(self) }
    fn ceil(self) -> f32 { libm::ceilf(self) }
    fn rem_euclid(self, rhs: f32) -> f32 {
        let r = self % rhs;
        if r < 0.0 { r + rhs.abs() } else { r }
    }
}


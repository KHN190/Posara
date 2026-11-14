#[cfg(any(feature = "sfx", test))]
#[inline]
pub(crate) fn unit_remainder(x: f32) -> f32 {
    let bits = x.to_bits();
    let magnitude = bits & 0x7fffffff;
    if magnitude < 0x3f800000 { return x; }
    if magnitude >= 0x7f800000 { return x % 1.0; }
    if magnitude >= 0x4b000000 { return f32::from_bits(bits & 0x80000000); }
    let positive = f32::from_bits(magnitude);
    (positive - (positive as u32) as f32).copysign(x)
}

#[cfg(any(feature = "synth", test))]
#[inline]
pub(crate) fn unit_euclid(x: f32) -> f32 {
    let remainder = unit_remainder(x);
    if remainder < 0.0 { remainder + 1.0 } else { remainder }
}

// Host audio has bit-exact recorded output; the bare-metal backend uses f32-only math.
pub fn sin(x: f32) -> f32 {
    #[cfg(any(feature = "device-dsp", all(target_os = "none", target_pointer_width = "32")))]
    { myriad::word32::fmath::sin(x) }
    #[cfg(not(any(feature = "device-dsp", all(target_os = "none", target_pointer_width = "32"))))]
    { libm::sinf(x) }
}
/// Sine of an oscillator phase in [0, 1]. The phase already carries the quadrant,
/// so no general radian argument reduction or library call is needed.
#[inline]
pub fn phase_sin(phase: f32) -> f32 {
    let quadrant = (phase * 4.0 + 0.5) as u32;
    let x = (phase - quadrant as f32 * 0.25) * core::f32::consts::TAU;
    let z = x * x;
    // On [-pi/4, pi/4], peak error is below 4e-6 (0.14 of a 16-bit PCM step).
    let value = if quadrant & 1 == 0 {
        x * (1.0 + z * (-1.0 / 6.0 + z * (1.0 / 120.0 - z / 5040.0)))
    } else {
        1.0 + z * (-0.5 + z * (1.0 / 24.0 - z / 720.0))
    };
    if quadrant & 2 == 0 { value } else { -value }
}

mod sine_lut;

/// Linearly interpolated sine for oscillator phases in [0, 1].
#[inline]
pub fn phase_sin_lut(phase: f32) -> f32 {
    let scaled = phase * 256.0;
    let index = scaled as usize;
    let fraction = scaled - index as f32;
    // Phase 1 is the same endpoint as phase 0; the last entry closes the cycle.
    let index = index & 255;
    let first = sine_lut::TABLE[index];
    first + (sine_lut::TABLE[index + 1] - first) * fraction
}

#[inline]
pub(crate) fn oscillator_sin(phase: f32) -> f32 {
    #[cfg(any(feature = "device-dsp", all(target_os = "none", target_pointer_width = "32")))]
    { phase_sin_lut(phase) }
    #[cfg(not(any(feature = "device-dsp", all(target_os = "none", target_pointer_width = "32"))))]
    { sin(phase * core::f32::consts::TAU) }
}

pub fn cos(x: f32) -> f32 {
    #[cfg(any(feature = "device-dsp", all(target_os = "none", target_pointer_width = "32")))]
    { myriad::word32::fmath::cos(x) }
    #[cfg(not(any(feature = "device-dsp", all(target_os = "none", target_pointer_width = "32"))))]
    { libm::cosf(x) }
}
#[cfg(any(feature = "gfx", feature = "sfx"))]
pub fn exp(x: f32) -> f32 { libm::expf(x) }
#[cfg(any(feature = "gfx", feature = "sfx"))]
pub fn tanh(x: f32) -> f32 { libm::tanhf(x) }
#[cfg(any(feature = "gfx", feature = "sfx"))]
pub fn powf(x: f32, n: f32) -> f32 {
    #[cfg(any(feature = "device-dsp", all(target_os = "none", target_pointer_width = "32")))]
    { pow32::powf(x, n) }
    #[cfg(not(any(feature = "device-dsp", all(target_os = "none", target_pointer_width = "32"))))]
    { libm::powf(x, n) }
}
#[cfg(any(feature = "device-dsp", all(target_os = "none", target_pointer_width = "32")))]
mod pow32;

#[cfg(all(test, not(any(feature = "device-dsp", all(target_os = "none", target_pointer_width = "32")))))]
mod host_math_tests {
    use super::*;

    #[test]
    fn host_math_preserves_the_recorded_libm_bits() {
        for i in -8192..=8192 {
            let x = i as f32 / 1024.0;
            assert_eq!(sin(x).to_bits(), libm::sinf(x).to_bits());
            assert_eq!(cos(x).to_bits(), libm::cosf(x).to_bits());
            assert_eq!(powf(2.0, x).to_bits(), libm::powf(2.0, x).to_bits());
        }
    }
}

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
    #[inline]
    fn sqrt(self) -> f32 {
        #[cfg(all(target_arch = "arm", target_os = "none", target_abi = "eabihf"))]
        {
            let mut bits = self.to_bits();
            // Explicit S0 clobber works with Rust's baseline hard-float ARM target.
            unsafe { core::arch::asm!("vmov s0, {bits}", "vsqrt.f32 s0, s0", "vmov {bits}, s0", bits = inout(reg) bits, out("s0") _, options(pure, nomem, nostack)); }
            f32::from_bits(bits)
        }
        #[cfg(not(all(target_arch = "arm", target_os = "none", target_abi = "eabihf")))]
        { myriad::word32::fmath::sqrt(self) }
    }
    fn ceil(self) -> f32 { libm::ceilf(self) }
    fn rem_euclid(self, rhs: f32) -> f32 {
        let r = self % rhs;
        if r < 0.0 { r + rhs.abs() } else { r }
    }
}


#[cfg(test)]
mod phase_tests {
    use super::*;

    fn check(x: f32) {
        let expected = x % 1.0;
        let actual = unit_remainder(x);
        if expected.is_nan() { assert!(actual.is_nan()); }
        else { assert_eq!(actual.to_bits(), expected.to_bits(), "x={x:?}"); }
        let expected = x.rem_euclid(1.0);
        let actual = unit_euclid(x);
        if expected.is_nan() { assert!(actual.is_nan()); }
        else { assert_eq!(actual.to_bits(), expected.to_bits(), "x={x:?}"); }
    }

    #[test]
    fn phase_reduction_matches_float_remainder_at_wraps_and_special_values() {
        for bits in [0, 0x80000000, 1, 0x80000001, 0x3f7fffff, 0x3f800000,
                     0x3f800001, 0xbf7fffff, 0xbf800000, 0xbf800001,
                     0x7f7fffff, 0xff7fffff, 0x7f800000, 0xff800000, 0x7fc01234] {
            check(f32::from_bits(bits));
        }
    }

    #[test]
    fn phase_reduction_matches_a_million_float_bit_patterns() {
        let mut bits = 0x12345678u32;
        for _ in 0..1_000_000 {
            bits = bits.wrapping_mul(1664525).wrapping_add(1013904223);
            check(f32::from_bits(bits));
        }
    }
}

#[cfg(test)]
mod sqrt_rounding_tests {
    extern crate std;
    #[test]
    fn ieee_sqrt_preserves_software_results_for_physical_inputs() {
        let mut bits = 1u32;
        for _ in 0..1_000_000 {
            bits = bits.wrapping_mul(1664525).wrapping_add(1013904223);
            let x = f32::from_bits(bits & 0x7fffffff);
            if x.is_finite() {
                assert_eq!(std::primitive::f32::sqrt(x).to_bits(), myriad::word32::fmath::sqrt(x).to_bits(), "{x:?}");
            }
        }
        for x in [0.0f32, -0.0, f32::INFINITY] {
            assert_eq!(std::primitive::f32::sqrt(x).to_bits(), myriad::word32::fmath::sqrt(x).to_bits());
        }
    }
}

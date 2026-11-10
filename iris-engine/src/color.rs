//! Color temperature math with L1-cache lookup table acceleration.

use crate::lut::{KELVIN_LUT, KELVIN_MAX, KELVIN_MIN, KELVIN_STEP};

/// Converts a color temperature in Kelvin (1000K - 10000K) to normalized RGB multipliers.
/// Uses precomputed L1-cached Blackbody / Tanner Helland values normalized to D65.
/// Sub-nanosecond O(1) execution with zero transcendental floating-point calls.
#[inline(always)]
pub fn kelvin_to_rgb(kelvin: u32) -> (f64, f64, f64) {
    let k = kelvin.clamp(KELVIN_MIN, KELVIN_MAX);
    let offset = k - KELVIN_MIN;
    let idx = (offset / KELVIN_STEP) as usize;
    let rem = offset % KELVIN_STEP;

    if rem == 0 || idx >= KELVIN_LUT.len() - 1 {
        let (r, g, b) = KELVIN_LUT[idx];
        (r as f64, g as f64, b as f64)
    } else {
        let (r0, g0, b0) = KELVIN_LUT[idx];
        let (r1, g1, b1) = KELVIN_LUT[idx + 1];
        let t = rem as f32 / KELVIN_STEP as f32;
        (
            (r0 + (r1 - r0) * t) as f64,
            (g0 + (g1 - g0) * t) as f64,
            (b0 + (b1 - b0) * t) as f64,
        )
    }
}

/// Computes final RGB scaling factors combining color temperature and software brightness.
/// - `kelvin`: 1000 to 10000 (Kelvin warmth)
/// - `brightness`: 0.05 to 1.0 (5% to 100%)
#[inline(always)]
pub fn calculate_multipliers(kelvin: u32, brightness: f64) -> (f64, f64, f64) {
    let (r, g, b) = kelvin_to_rgb(kelvin);
    let brightness = brightness.clamp(0.05, 1.0);
    (r * brightness, g * brightness, b * brightness)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daylight_normalization() {
        let (r, g, b) = kelvin_to_rgb(6500);
        assert!((r - 1.0).abs() < 0.001, "Red at 6500K should be ~1.0: {}", r);
        assert!((g - 1.0).abs() < 0.001, "Green at 6500K should be ~1.0: {}", g);
        assert!((b - 1.0).abs() < 0.001, "Blue at 6500K should be ~1.0: {}", b);
    }

    #[test]
    fn test_warm_night_reduction() {
        let (r, g, b) = kelvin_to_rgb(3400);
        assert!(r > 0.95, "Red should remain strong: {}", r);
        assert!(g > 0.65 && g < 0.85, "Green should be moderately reduced: {}", g);
        assert!(b < 0.60, "Blue should be strongly reduced: {}", b);
    }

    #[test]
    fn test_candlelight_blue_cutoff() {
        let (_r, _g, b) = kelvin_to_rgb(1800);
        assert_eq!(b, 0.0, "Blue should be zero at <1900K");
    }

    #[test]
    fn test_brightness_scaling() {
        let (r, g, b) = calculate_multipliers(6500, 0.5);
        assert!((r - 0.5).abs() < 0.02);
        assert!((g - 0.5).abs() < 0.02);
        assert!((b - 0.5).abs() < 0.02);
    }

    #[test]
    fn test_sub_step_interpolation() {
        let (r1, g1, b1) = kelvin_to_rgb(3400);
        let (r2, g2, b2) = kelvin_to_rgb(3405);
        let (r3, g3, b3) = kelvin_to_rgb(3410);
        assert!(r2 >= r1.min(r3) && r2 <= r1.max(r3));
        assert!(g2 >= g1.min(g3) && g2 <= g1.max(g3));
        assert!(b2 >= b1.min(b3) && b2 <= b1.max(b3));
    }
}

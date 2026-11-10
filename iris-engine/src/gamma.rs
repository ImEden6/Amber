//! Windows GDI Gamma Ramp representation and smooth interpolation.

/// Exact binary layout expected by Win32 `GetDeviceGammaRamp` / `SetDeviceGammaRamp`.
/// Layout: 3 channels (Red, Green, Blue) of 256 16-bit words each (1536 bytes total).
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GammaRamp {
    pub red: [u16; 256],
    pub green: [u16; 256],
    pub blue: [u16; 256],
}

impl Default for GammaRamp {
    fn default() -> Self {
        Self::linear()
    }
}

impl GammaRamp {
    /// Generates a pristine, unmodified linear gamma curve (0 to 65535).
    pub fn linear() -> Self {
        let mut ramp = Self {
            red: [0; 256],
            green: [0; 256],
            blue: [0; 256],
        };

        for i in 0..256 {
            let val = (i as u32 * 257) as u16;
            ramp.red[i] = val;
            ramp.green[i] = val;
            ramp.blue[i] = val;
        }

        ramp
    }

    pub fn from_multipliers(
        r_mult: f64,
        g_mult: f64,
        b_mult: f64,
        base_ramp: Option<&GammaRamp>,
    ) -> Self {
        let linear = Self::linear();
        let base = base_ramp.unwrap_or(&linear);

        let r_fixed = (r_mult.clamp(0.0, 1.0) * 65536.0).round() as u32;
        let g_fixed = (g_mult.clamp(0.0, 1.0) * 65536.0).round() as u32;
        let b_fixed = (b_mult.clamp(0.0, 1.0) * 65536.0).round() as u32;

        let mut out = Self {
            red: [0; 256],
            green: [0; 256],
            blue: [0; 256],
        };

        for i in 0..256 {
            out.red[i] = ((base.red[i] as u32 * r_fixed + 32768) >> 16).min(65535) as u16;
            out.green[i] = ((base.green[i] as u32 * g_fixed + 32768) >> 16).min(65535) as u16;
            out.blue[i] = ((base.blue[i] as u32 * b_fixed + 32768) >> 16).min(65535) as u16;
        }

        out
    }

    pub fn lerp(&self, target: &GammaRamp, factor: f64) -> Self {
        let factor = factor.clamp(0.0, 1.0);
        let t = (1.0 - (factor * std::f64::consts::PI).cos()) / 2.0;
        let t_fixed = (t * 65536.0).round() as i64;

        let mut out = Self {
            red: [0; 256],
            green: [0; 256],
            blue: [0; 256],
        };

        for i in 0..256 {
            out.red[i] = Self::interpolate_fixed(self.red[i], target.red[i], t_fixed);
            out.green[i] = Self::interpolate_fixed(self.green[i], target.green[i], t_fixed);
            out.blue[i] = Self::interpolate_fixed(self.blue[i], target.blue[i], t_fixed);
        }

        out
    }

    #[inline(always)]
    fn interpolate_fixed(from: u16, to: u16, t_fixed: i64) -> u16 {
        let f = from as i64;
        let diff = (to as i64) - f;
        let res = f + ((diff * t_fixed + 32768) >> 16);
        res.clamp(0, 65535) as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_ramp() {
        let ramp = GammaRamp::linear();
        assert_eq!(ramp.red[0], 0);
        assert_eq!(ramp.green[0], 0);
        assert_eq!(ramp.blue[0], 0);
        assert_eq!(ramp.red[255], 65535);
        assert_eq!(ramp.green[255], 65535);
        assert_eq!(ramp.blue[255], 65535);
    }

    #[test]
    fn test_scaled_ramp() {
        let scaled = GammaRamp::from_multipliers(1.0, 0.5, 0.0, None);
        assert_eq!(scaled.red[255], 65535);
        assert_eq!(scaled.green[255], 32768);
        assert_eq!(scaled.blue[255], 0);
    }

    #[test]
    fn test_smooth_lerp() {
        let a = GammaRamp::linear();
        let b = GammaRamp::from_multipliers(0.0, 0.0, 0.0, None);

        let mid = a.lerp(&b, 0.5);
        assert_eq!(mid.red[255], 32768);

        let start = a.lerp(&b, 0.0);
        assert_eq!(start.red[255], 65535);

        let end = a.lerp(&b, 1.0);
        assert_eq!(end.red[255], 0);
    }
}

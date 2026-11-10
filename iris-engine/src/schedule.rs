//! Time-of-day schedule for automatic smooth transitions between Day, Sunset, and Sleep.

/// Configuration for the day/night/sleep automatic cycle.
#[derive(Debug, Clone, Copy)]
pub struct ScheduleConfig {
    /// Hour and minute the day starts (e.g. 7:00).
    pub day_time: (u8, u8),
    /// Day color temperature (e.g. 6000K or 6500K).
    pub day_temp: u32,
    /// Day brightness (1.0 = 100%).
    pub day_brightness: f64,

    /// Hour and minute sunset begins (e.g. 19:00 / 7:00 PM).
    pub sunset_time: (u8, u8),
    /// Night color temperature (e.g. 3400K).
    pub night_temp: u32,
    /// Night brightness (e.g. 0.85).
    pub night_brightness: f64,

    /// Hour and minute bedtime begins (e.g. 23:00 / 11:00 PM).
    pub bedtime: (u8, u8),
    /// Sleep/Candlelight color temperature (e.g. 2200K).
    pub sleep_temp: u32,
    /// Sleep brightness (e.g. 0.70).
    pub sleep_brightness: f64,

    /// Duration of transitions in minutes (e.g. 45 minutes of gradual shifting).
    pub transition_duration_minutes: u32,
}

impl Default for ScheduleConfig {
    fn default() -> Self {
        Self {
            day_time: (7, 0),
            day_temp: 6500,
            day_brightness: 1.0,

            sunset_time: (19, 0),
            night_temp: 3400,
            night_brightness: 0.85,

            bedtime: (23, 0),
            sleep_temp: 2200,
            sleep_brightness: 0.70,

            transition_duration_minutes: 60,
        }
    }
}

impl ScheduleConfig {
    /// Computes the exact (temperature, brightness) targets for a given local time `(hour, minute)`.
    pub fn calculate_targets(&self, hour: u8, minute: u8) -> (u32, f64) {
        let current_mins = (hour as i32) * 60 + (minute as i32);

        let day_mins = (self.day_time.0 as i32) * 60 + (self.day_time.1 as i32);
        let sunset_mins = (self.sunset_time.0 as i32) * 60 + (self.sunset_time.1 as i32);
        let bed_mins = (self.bedtime.0 as i32) * 60 + (self.bedtime.1 as i32);
        let trans = self.transition_duration_minutes as i32;

        if current_mins >= (day_mins - trans) && current_mins < day_mins {
            let factor = (current_mins - (day_mins - trans)) as f64 / (trans as f64);
            return Self::lerp_targets(
                self.sleep_temp, self.sleep_brightness,
                self.day_temp, self.day_brightness,
                factor,
            );
        }

        if current_mins >= day_mins && current_mins < sunset_mins {
            return (self.day_temp, self.day_brightness);
        }

        if current_mins >= sunset_mins && current_mins < (sunset_mins + trans) {
            let factor = (current_mins - sunset_mins) as f64 / (trans as f64);
            return Self::lerp_targets(
                self.day_temp, self.day_brightness,
                self.night_temp, self.night_brightness,
                factor,
            );
        }

        if current_mins >= (sunset_mins + trans) && current_mins < bed_mins {
            return (self.night_temp, self.night_brightness);
        }

        if current_mins >= bed_mins && current_mins < (bed_mins + trans) {
            let factor = (current_mins - bed_mins) as f64 / (trans as f64);
            return Self::lerp_targets(
                self.night_temp, self.night_brightness,
                self.sleep_temp, self.sleep_brightness,
                factor,
            );
        }

        (self.sleep_temp, self.sleep_brightness)
    }

    fn lerp_targets(
        temp_a: u32, bright_a: f64,
        temp_b: u32, bright_b: f64,
        factor: f64,
    ) -> (u32, f64) {
        let factor = factor.clamp(0.0, 1.0);
        let t = (1.0 - (factor * std::f64::consts::PI).cos()) / 2.0;

        let temp = (temp_a as f64 + (temp_b as f64 - temp_a as f64) * t).round() as u32;
        let bright = bright_a + (bright_b - bright_a) * t;

        (temp, bright)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_midday_target() {
        let schedule = ScheduleConfig::default();
        let (temp, bright) = schedule.calculate_targets(13, 0);
        assert_eq!(temp, 6500);
        assert_eq!(bright, 1.0);
    }

    #[test]
    fn test_midnight_target() {
        let schedule = ScheduleConfig::default();
        let (temp, bright) = schedule.calculate_targets(1, 30);
        assert_eq!(temp, 2200);
        assert_eq!(bright, 0.70);
    }

    #[test]
    fn test_sunset_mid_transition() {
        let schedule = ScheduleConfig::default();
        let (temp, bright) = schedule.calculate_targets(19, 30);
        assert!(temp < 6500 && temp > 3400, "Midpoint temp: {}", temp);
        assert!(bright < 1.0 && bright > 0.85, "Midpoint bright: {}", bright);
    }
}

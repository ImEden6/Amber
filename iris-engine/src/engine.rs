use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use crate::color::kelvin_to_rgb;
use crate::gamma::GammaRamp;
use crate::overlay::ScreenDimmer;
use crate::platform::{get_gamma_ramp, set_gamma_ramp, GammaSession};

pub struct DisplayEngine {
    initial_ramp: GammaRamp,
    current_ramp: Arc<Mutex<GammaRamp>>,
    target_kelvin: u32,
    target_brightness: f64,
    is_active: bool,
    transition_ms: u64,
    anim_token: Arc<AtomicU64>,
    dimmer: ScreenDimmer,
}

impl DisplayEngine {
    pub fn new() -> Result<Self, String> {
        let initial_ramp = get_gamma_ramp()
            .ok()
            .filter(|r| !r.is_tinted())
            .unwrap_or_else(GammaRamp::linear);

        Ok(Self {
            initial_ramp,
            current_ramp: Arc::new(Mutex::new(initial_ramp)),
            target_kelvin: 6500,
            target_brightness: 1.0,
            is_active: false,
            transition_ms: 350,
            anim_token: Arc::new(AtomicU64::new(0)),
            dimmer: ScreenDimmer::new(),
        })
    }

    pub fn initial_ramp(&self) -> &GammaRamp {
        &self.initial_ramp
    }

    pub fn target_kelvin(&self) -> u32 {
        self.target_kelvin
    }

    pub fn target_brightness(&self) -> f64 {
        self.target_brightness
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn set_transition_ms(&mut self, ms: u64) {
        self.transition_ms = ms;
    }

    pub fn set_temperature(&mut self, kelvin: u32, smooth: bool) -> Result<(), String> {
        self.set_target(kelvin, self.target_brightness, smooth)
    }

    pub fn set_brightness(&mut self, brightness: f64, smooth: bool) -> Result<(), String> {
        self.set_target(self.target_kelvin, brightness, smooth)
    }

    pub fn set_target(&mut self, kelvin: u32, brightness: f64, smooth: bool) -> Result<(), String> {
        self.target_kelvin = kelvin.clamp(1000, 10000);
        self.target_brightness = brightness.clamp(0.10, 1.0);
        self.is_active = true;

        let dim_ratio = (1.0 - self.target_brightness).max(0.0);
        self.dimmer.set_dimming(dim_ratio);

        let (r, g, b) = kelvin_to_rgb(self.target_kelvin);
        let safe_r = r;
        let safe_g = g.max(0.55);
        let safe_b = b.max(0.48);
        let target_ramp = GammaRamp::from_multipliers(safe_r, safe_g, safe_b, Some(&self.initial_ramp));

        if smooth && self.transition_ms > 0 {
            self.animate_to(target_ramp, self.transition_ms)
        } else {
            self.anim_token.fetch_add(1, Ordering::SeqCst);
            if let Ok(mut g) = self.current_ramp.lock() {
                if *g != target_ramp {
                    let _ = set_gamma_ramp(&target_ramp);
                    *g = target_ramp;
                }
            } else {
                let _ = set_gamma_ramp(&target_ramp);
            }
            Ok(())
        }
    }

    pub fn toggle_active(&mut self, smooth: bool) -> Result<bool, String> {
        if self.is_active {
            self.pause(smooth)?;
            Ok(false)
        } else {
            self.resume(smooth)?;
            Ok(true)
        }
    }

    pub fn pause(&mut self, smooth: bool) -> Result<(), String> {
        self.is_active = false;
        self.dimmer.clear();
        let initial = self.initial_ramp;
        if smooth && self.transition_ms > 0 {
            self.animate_to(initial, self.transition_ms)
        } else {
            self.anim_token.fetch_add(1, Ordering::SeqCst);
            let _ = set_gamma_ramp(&initial);
            if let Ok(mut g) = self.current_ramp.lock() {
                *g = initial;
            }
            Ok(())
        }
    }

    pub fn resume(&mut self, smooth: bool) -> Result<(), String> {
        self.set_target(self.target_kelvin, self.target_brightness, smooth)
    }

    pub fn refresh(&self) -> Result<(), String> {
        if self.is_active {
            let dim_ratio = (1.0 - self.target_brightness).max(0.0);
            self.dimmer.set_dimming(dim_ratio);
        }
        if let Ok(ramp) = self.current_ramp.lock() {
            let _ = set_gamma_ramp(&ramp);
        }
        Ok(())
    }

    pub fn restore_initial(&mut self) -> Result<(), String> {
        self.is_active = false;
        self.dimmer.clear();
        self.anim_token.fetch_add(1, Ordering::SeqCst);
        let _ = set_gamma_ramp(&self.initial_ramp);
        if let Ok(mut g) = self.current_ramp.lock() {
            *g = self.initial_ramp;
        }
        Ok(())
    }

    fn animate_to(&mut self, target: GammaRamp, duration_ms: u64) -> Result<(), String> {
        let token = Arc::clone(&self.anim_token);
        let generation = token.fetch_add(1, Ordering::SeqCst) + 1;
        let start_ramp = *self.current_ramp.lock().map_err(|e| e.to_string())?;
        let current_ramp_ref = Arc::clone(&self.current_ramp);

        thread::spawn(move || {
            let session = match GammaSession::open() {
                Ok(s) => s,
                Err(_) => return,
            };

            let start_time = Instant::now();
            let total_duration = Duration::from_millis(duration_ms);
            let frame_interval = Duration::from_millis(16);
            let mut last_applied = start_ramp;

            loop {
                if token.load(Ordering::Relaxed) != generation {
                    return;
                }
                let elapsed = start_time.elapsed();
                if elapsed >= total_duration {
                    break;
                }

                let factor = elapsed.as_secs_f64() / total_duration.as_secs_f64();
                let intermediate = start_ramp.lerp(&target, factor);
                if intermediate != last_applied {
                    let _ = session.set_ramp(&intermediate);
                    last_applied = intermediate;
                }

                if let Ok(mut g) = current_ramp_ref.lock() {
                    *g = intermediate;
                }

                thread::sleep(frame_interval);
            }

            if token.load(Ordering::Relaxed) == generation {
                if target != last_applied {
                    let _ = session.set_ramp(&target);
                }
                if let Ok(mut g) = current_ramp_ref.lock() {
                    *g = target;
                }
            }
        });

        Ok(())
    }
}

impl Drop for DisplayEngine {
    fn drop(&mut self) {
        self.anim_token.fetch_add(1, Ordering::SeqCst);
        self.dimmer.clear();
        let _ = set_gamma_ramp(&self.initial_ramp);
    }
}

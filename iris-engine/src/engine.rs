use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
use crate::color::kelvin_to_rgb;
use crate::gamma::GammaRamp;
use crate::overlay::ScreenDimmer;
use crate::platform::{get_gamma_ramp, set_gamma_ramp, GammaSession};

static SHUTDOWN_RESTORE: OnceLock<(GammaRamp, PathBuf)> = OnceLock::new();

pub(crate) fn restore_on_shutdown() {
    if let Some((ramp, path)) = SHUTDOWN_RESTORE.get() {
        let _ = set_gamma_ramp(ramp);
        let _ = fs::remove_file(path);
    }
}

fn default_state_path() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Amber")
        .join("gamma-baseline.bin")
}

fn resolve_baseline(path: &Path, current: GammaRamp) -> GammaRamp {
    let baseline = read_ramp(path).unwrap_or(current);
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let bytes: Vec<u8> = baseline.red.iter()
        .chain(&baseline.green)
        .chain(&baseline.blue)
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let _ = fs::write(path, bytes);
    baseline
}

fn read_ramp(path: &Path) -> Option<GammaRamp> {
    let bytes = fs::read(path).ok()?;
    if bytes.len() != 1536 {
        return None;
    }
    let mut words = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]]));
    let mut ramp = GammaRamp::linear();
    for channel in [&mut ramp.red, &mut ramp.green, &mut ramp.blue] {
        for v in channel.iter_mut() {
            *v = words.next()?;
        }
    }
    Some(ramp)
}

pub struct DisplayEngine {
    initial_ramp: GammaRamp,
    current_ramp: Arc<Mutex<GammaRamp>>,
    target_kelvin: u32,
    target_brightness: f64,
    is_active: bool,
    transition_ms: u64,
    anim_token: Arc<AtomicU64>,
    dimmer: ScreenDimmer,
    state_path: PathBuf,
}

impl DisplayEngine {
    pub fn new() -> Result<Self, String> {
        let state_path = default_state_path();
        let current = get_gamma_ramp().unwrap_or_else(|_| GammaRamp::linear());
        let initial_ramp = resolve_baseline(&state_path, current);
        let _ = SHUTDOWN_RESTORE.set((initial_ramp, state_path.clone()));

        Ok(Self {
            initial_ramp,
            current_ramp: Arc::new(Mutex::new(initial_ramp)),
            target_kelvin: 6500,
            target_brightness: 1.0,
            is_active: false,
            transition_ms: 350,
            anim_token: Arc::new(AtomicU64::new(0)),
            dimmer: ScreenDimmer::new(),
            state_path,
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
            let mut g = self.current_ramp.lock().unwrap_or_else(|e| e.into_inner());
            self.anim_token.fetch_add(1, Ordering::SeqCst);
            if *g != target_ramp {
                let _ = set_gamma_ramp(&target_ramp);
                *g = target_ramp;
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
            let mut g = self.current_ramp.lock().unwrap_or_else(|e| e.into_inner());
            self.anim_token.fetch_add(1, Ordering::SeqCst);
            let _ = set_gamma_ramp(&initial);
            *g = initial;
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
        let mut g = self.current_ramp.lock().unwrap_or_else(|e| e.into_inner());
        self.anim_token.fetch_add(1, Ordering::SeqCst);
        let _ = set_gamma_ramp(&self.initial_ramp);
        *g = self.initial_ramp;
        drop(g);
        let _ = fs::remove_file(&self.state_path);
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
                let elapsed = start_time.elapsed();
                if elapsed >= total_duration {
                    break;
                }

                let factor = elapsed.as_secs_f64() / total_duration.as_secs_f64();
                let intermediate = start_ramp.lerp(&target, factor);
                {
                    let mut g = current_ramp_ref.lock().unwrap_or_else(|e| e.into_inner());
                    if token.load(Ordering::SeqCst) != generation {
                        return;
                    }
                    if intermediate != last_applied {
                        let _ = session.set_ramp(&intermediate);
                        last_applied = intermediate;
                    }
                    *g = intermediate;
                }

                thread::sleep(frame_interval);
            }

            let mut g = current_ramp_ref.lock().unwrap_or_else(|e| e.into_inner());
            if token.load(Ordering::SeqCst) == generation {
                if target != last_applied {
                    let _ = session.set_ramp(&target);
                }
                *g = target;
            }
        });

        Ok(())
    }
}

impl Drop for DisplayEngine {
    fn drop(&mut self) {
        let _ = self.restore_initial();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_baseline_survives_unclean_exit() {
        let path = std::env::temp_dir().join(format!("amber-baseline-test-{}.bin", std::process::id()));
        let _ = fs::remove_file(&path);

        let calibrated = GammaRamp::from_multipliers(1.0, 0.9, 0.8, None);
        assert_eq!(resolve_baseline(&path, calibrated), calibrated);

        let leftover = GammaRamp::from_multipliers(1.0, 0.95, 0.92, Some(&calibrated));
        assert_eq!(resolve_baseline(&path, leftover), calibrated);

        fs::remove_file(&path).unwrap();
        assert_eq!(resolve_baseline(&path, leftover), leftover);

        let _ = fs::remove_file(&path);
    }
}

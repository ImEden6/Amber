use iris_engine::{DisplayEngine, ScheduleConfig};
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Iris-Engine Core Demo ===");
    
    // 1. Initialize engine (safely captures original display profile)
    println!("Initializing engine & capturing monitor calibration...");
    let mut engine = DisplayEngine::new()?;
    println!("Engine ready! Filter active: {}", engine.is_active());

    // 2. Demonstrate schedule calculation
    let schedule = ScheduleConfig::default();
    println!("\nTesting schedule calculations:");
    let times = [(8, 0, "Morning"), (13, 0, "Midday"), (19, 30, "Sunset"), (22, 0, "Night"), (1, 0, "Sleep")];
    for (h, m, label) in times {
        let (temp, bright) = schedule.calculate_targets(h, m);
        println!("  - {:02}:{:02} ({:<7}): {:4}K | Brightness: {:.0}%", h, m, label, temp, bright * 100.0);
    }

    // 3. Smooth transition to warm sunset temperature (3800K) and 90% brightness
    println!("\nSmoothly transitioning to 3800K (Warm sunset) at 90% brightness...");
    engine.set_target(3800, 0.90, true)?;
    println!("Warm filter active! Waiting 2 seconds...");
    thread::sleep(Duration::from_secs(2));

    // 4. Smooth transition to sleep candlelight (2400K) and 75% brightness
    println!("\nSmoothly transitioning to 2400K (Bedtime / Candlelight) at 75% brightness...");
    engine.set_target(2400, 0.75, true)?;
    println!("Bedtime filter active! Waiting 2 seconds...");
    thread::sleep(Duration::from_secs(2));

    // 5. Smoothly restoring natural screen profile
    println!("\nSmoothly restoring natural display calibration...");
    engine.pause(true)?;
    println!("Natural calibration restored!");

    println!("\nDemo completed successfully. Zero bloat, zero OpenCV, 100% native.");
    Ok(())
}

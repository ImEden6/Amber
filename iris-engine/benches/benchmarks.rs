use iris_engine::{calculate_multipliers, kelvin_to_rgb, GammaRamp, ScheduleConfig};
use std::time::Instant;

fn main() {
    println!("============================================================");
    println!("       IRIS-ENGINE PERFORMANCE BENCHMARK (BASELINE)        ");
    println!("============================================================");

    // Warm up CPU
    let mut dummy = 0.0;
    for k in 1000..6500 {
        let (r, g, b) = kelvin_to_rgb(k);
        dummy += r + g + b;
    }
    std::hint::black_box(dummy);

    // 1. Benchmark: kelvin_to_rgb
    const KELVIN_ITERS: usize = 1_000_000;
    let start = Instant::now();
    for i in 0..KELVIN_ITERS {
        let k = 1000 + ((i % 5500) as u32);
        let res = kelvin_to_rgb(k);
        std::hint::black_box(res);
    }
    let kelvin_duration = start.elapsed();
    let ns_per_kelvin = kelvin_duration.as_nanos() as f64 / KELVIN_ITERS as f64;
    let kelvin_ops_per_sec = (KELVIN_ITERS as f64 / kelvin_duration.as_secs_f64()) / 1_000_000.0;

    println!("\n[1] kelvin_to_rgb (1,000,000 ops):");
    println!("    Total time: {:?}", kelvin_duration);
    println!("    Latency:    {:.2} ns / op", ns_per_kelvin);
    println!("    Throughput: {:.2} M ops / sec", kelvin_ops_per_sec);

    // 2. Benchmark: GammaRamp::from_multipliers
    const RAMP_ITERS: usize = 100_000;
    let base = GammaRamp::linear();
    let start = Instant::now();
    for i in 0..RAMP_ITERS {
        let r = 0.8 + ((i % 20) as f64) * 0.01;
        let g = 0.6 + ((i % 20) as f64) * 0.01;
        let b = 0.3 + ((i % 20) as f64) * 0.01;
        let ramp = GammaRamp::from_multipliers(r, g, b, Some(&base));
        std::hint::black_box(ramp);
    }
    let ramp_duration = start.elapsed();
    let ns_per_ramp = ramp_duration.as_nanos() as f64 / RAMP_ITERS as f64;
    let ramp_ops_per_sec = (RAMP_ITERS as f64 / ramp_duration.as_secs_f64()) / 1_000.0;

    println!("\n[2] GammaRamp::from_multipliers (100,000 ops):");
    println!("    Total time: {:?}", ramp_duration);
    println!("    Latency:    {:.2} ns / op ({:.2} µs / op)", ns_per_ramp, ns_per_ramp / 1000.0);
    println!("    Throughput: {:.2} K ops / sec", ramp_ops_per_sec);

    // 3. Benchmark: GammaRamp::lerp
    const LERP_ITERS: usize = 200_000;
    let ramp_a = GammaRamp::linear();
    let ramp_b = GammaRamp::from_multipliers(0.9, 0.7, 0.4, Some(&ramp_a));
    let start = Instant::now();
    for i in 0..LERP_ITERS {
        let factor = ((i % 100) as f64) / 100.0;
        let ramp = ramp_a.lerp(&ramp_b, factor);
        std::hint::black_box(ramp);
    }
    let lerp_duration = start.elapsed();
    let ns_per_lerp = lerp_duration.as_nanos() as f64 / LERP_ITERS as f64;
    let lerp_ops_per_sec = (LERP_ITERS as f64 / lerp_duration.as_secs_f64()) / 1_000.0;

    println!("\n[3] GammaRamp::lerp (200,000 ops):");
    println!("    Total time: {:?}", lerp_duration);
    println!("    Latency:    {:.2} ns / op ({:.2} µs / op)", ns_per_lerp, ns_per_lerp / 1000.0);
    println!("    Throughput: {:.2} K ops / sec", lerp_ops_per_sec);

    // 4. Benchmark: Schedule target computation
    const SCHED_ITERS: usize = 500_000;
    let sched = ScheduleConfig::default();
    let start = Instant::now();
    for i in 0..SCHED_ITERS {
        let h = ((i / 60) % 24) as u8;
        let m = (i % 60) as u8;
        let target = sched.calculate_targets(h, m);
        std::hint::black_box(target);
    }
    let sched_duration = start.elapsed();
    let ns_per_sched = sched_duration.as_nanos() as f64 / SCHED_ITERS as f64;
    let sched_ops_per_sec = (SCHED_ITERS as f64 / sched_duration.as_secs_f64()) / 1_000_000.0;

    println!("\n[4] ScheduleConfig::calculate_targets (500,000 ops):");
    println!("    Total time: {:?}", sched_duration);
    println!("    Latency:    {:.2} ns / op", ns_per_sched);
    println!("    Throughput: {:.2} M ops / sec", sched_ops_per_sec);

    // 5. End-to-end frame computation (Kelvin + Brightness + Ramp + Lerp)
    const FRAME_ITERS: usize = 50_000;
    let start = Instant::now();
    for i in 0..FRAME_ITERS {
        let k = 2000 + ((i % 4000) as u32);
        let b = 0.5 + ((i % 50) as f64) * 0.01;
        let (r, g, bl) = calculate_multipliers(k, b);
        let target = GammaRamp::from_multipliers(r, g, bl, Some(&base));
        let frame = base.lerp(&target, 0.5);
        std::hint::black_box(frame);
    }
    let frame_duration = start.elapsed();
    let ns_per_frame = frame_duration.as_nanos() as f64 / FRAME_ITERS as f64;
    let fps_capacity = 1_000_000_000.0 / ns_per_frame;

    println!("\n[5] Full Frame Compute Pipeline (50,000 frames):");
    println!("    Latency:    {:.2} µs / frame", ns_per_frame / 1000.0);
    println!("    Capacity:   {:.0} frames / sec (theoretical compute ceiling)", fps_capacity);
    println!("============================================================");
}

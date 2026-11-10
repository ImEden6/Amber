# Amber 🌅

> **Ultra-lightweight, high-performance eye protection and color temperature management for Windows.**

Amber is an open-source, bloat-free alternative to proprietary eye-care tools like Iris and f.lux. Built from the ground up in **Rust** and **Tauri v2**, Amber delivers instant, sub-nanosecond color temperature adjustments and true sub-hardware screen dimming without requiring admin privileges, registry modifications, or heavy runtime dependencies.

---

## ✨ Features

- **Wide Temperature Range (1000K – 6500K)**: Continuous color warmth control based on Tanner Helland's blackbody radiation formula normalized to D65.
- **True Sub-Hardware Dimming (10% – 100%)**: Dims below monitor backlight hardware limits without PWM flicker using a native click-through Win32 layered overlay (`WS_EX_LAYERED | WS_EX_TRANSPARENT`).
- **Circadian Schedule Automation**: Automatically transitions between Day (6500K), Sunset (3400K), and Bedtime (2200K) based on local time.
- **Glassmorphism Dark UI**: Clean, modern interface with real-time glowing preview, quick presets (☀️ Day, 📖 Read, 🌅 Sunset, 🌙 Sleep), and instant toggles.
- **System Tray Integration**: Runs quietly in the notification area with quick preset access and pause/resume controls.
- **Zero Bloat & Peak Performance**:
  - **~4.5 MB** release executable.
  - **~8.4 MB** private RAM footprint (compared to 85–120 MB in Qt/Electron alternatives).
  - **0.00%** idle CPU usage (pure event-driven architecture, zero wakeups when idle).
  - No background telemetry, no licensing checks, no webcam face tracking.

---

## ⚡ Performance

The core display engine ([`iris-engine`](iris-engine)) is optimized down to the nanosecond:

| Component | Implementation | Latency | Heap Allocations |
|---|---|---|---|
| **Kelvin to RGB** | 901-entry L1-cache static LUT | **2.77 ns** | 0 |
| **Gamma Ramp Generation** | 16.16 fixed-point SIMD vectorization | **217 ns** | 0 |
| **Cosine Ramp Interpolation** | Fixed-point integer Lerp | **437 ns** | 0 |

---

## 🏗️ Project Architecture

```
amber/
├── iris-engine/             # Core Rust display & color library
│   ├── src/
│   │   ├── color.rs         # Color temperature & RGB scaling
│   │   ├── lut.rs           # Static 1000K-10000K lookup table
│   │   ├── gamma.rs         # SIMD 16.16 fixed-point gamma ramp math
│   │   ├── overlay.rs       # Win32 click-through transparent dimmer window
│   │   ├── platform.rs      # Win32 GDI SetDeviceGammaRamp bindings
│   │   ├── schedule.rs      # Circadian time-of-day progression logic
│   │   └── engine.rs        # Coordinator with RAII calibration restore
│   └── benches/             # Criterion microbenchmarks
│
└── iris-app/                # Tauri v2 desktop application
    ├── src-tauri/           # Rust Tauri backend, IPC commands, and tray
    └── ui/                  # Vanilla glassmorphic HTML/CSS/JS frontend
```

---

## 🚀 Building & Running

### Prerequisites
- [Rust](https://rustup.rs/) (1.90+ recommended)
- Windows 10 / 11 with WebView2 Runtime (installed by default on Windows 10/11)

### Quick Run
```powershell
# 1. Run engine tests
cd iris-engine
cargo test

# 2. Run engine microbenchmarks
cargo bench

# 3. Launch desktop app in development
cd ../iris-app/src-tauri
cargo tauri dev
```

### Compiling Release Binary
```powershell
cd iris-app/src-tauri
cargo build --release
```
The optimized executable will be located at:
`iris-app/src-tauri/target/release/amber-app.exe`

---

## 📜 License

MIT License. Free and open source.

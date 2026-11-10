//! Iris-Engine: Ultra-lightweight, bloat-free display color temperature & gamma engine.

pub mod color;
pub mod gamma;
pub mod platform;
pub mod engine;
pub mod schedule;
pub mod overlay;
pub mod lut;

pub use color::{calculate_multipliers, kelvin_to_rgb};
pub use gamma::GammaRamp;
pub use engine::DisplayEngine;
pub use schedule::ScheduleConfig;
pub use overlay::ScreenDimmer;
pub use platform::GammaSession;

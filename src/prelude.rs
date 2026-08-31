//! 常用重导出 — `use audio_switcher_rust::prelude::*;`.

pub use crate::audio::{AudioBackend, AudioDevice, AudioError, AudioSnapshot};
pub use crate::config::{clamp_volume, AppConfig, Lang};
pub use crate::platform::{ComGuard, SingleInstanceGuard};
pub use crate::ui::{TrayWrapper, WheelState};

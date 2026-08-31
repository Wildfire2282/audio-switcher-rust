//! Prelude — commonly used types. `use audio_switcher_rust::prelude::*;`.

/// Commonly used audio and config types.
pub use crate::audio::{AudioBackend, AudioDevice};
pub use crate::config::{clamp_volume, AppConfig, Lang};
/// Platform guards for binary entry.
pub use crate::platform::{ComGuard, SingleInstanceGuard};

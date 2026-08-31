//! UI layer — tray icon, menu, tooltip and wheel handling.

pub mod i18n;
pub mod icon;
pub mod menu;
pub mod tooltip;
pub mod tray;
pub mod wheel;

/// Re-exported for `crate::app`.
pub use tooltip::format_tooltip;
pub use tray::{open_sound_settings, open_volume_mixer, TrayWrapper};
pub use wheel::WheelState;

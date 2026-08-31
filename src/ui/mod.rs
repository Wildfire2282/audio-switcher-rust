//! UI layer — tray icon, menu, tooltip and wheel handling.

/// Internationalisation helper.
pub mod i18n;
/// Icon rendering.
pub mod icon;
/// Tray menu builder.
pub mod menu;
/// Tooltip formatting.
pub mod tooltip;
/// Tray wrapper.
pub mod tray;
/// Wheel acceleration state.
pub mod wheel;

/// Re-exported for `crate::app`.
pub use tooltip::format_tooltip;
pub use tray::{open_sound_settings, open_volume_mixer, TrayWrapper};
pub use wheel::WheelState;

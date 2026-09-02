//! UI layer — tray icon, menu, tooltip and wheel handling.

/// Internationalisation helper.
pub mod i18n;
/// Icon rendering.
pub mod icon;
/// Tray menu builder.
pub mod menu;
/// Shared label sanitization/truncation for menu + tooltip.
pub(crate) mod text;
/// Tooltip formatting.
pub mod tooltip;
/// Tray wrapper.
pub mod tray;
/// Wheel acceleration state.
pub mod wheel;

/// Re-exported for `crate::app`.
pub use tooltip::format_tooltip;
pub use tray::{TrayWrapper, open_sound_settings, open_volume_mixer};
pub use wheel::WheelState;

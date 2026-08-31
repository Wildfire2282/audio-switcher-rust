//! Backwards-compatible facade. New code lives in `crate::ui::*`.
//! Keeps `crate::tray::*` imports working for existing callers/tests.
#![allow(unused_imports)]

pub use crate::ui::i18n::tr;
pub use crate::ui::icon::make_icon;
pub use crate::ui::menu::{build_menu, MenuHandles};
pub use crate::ui::theme::{invalidate_dark_mode_cache, is_dark_mode};
pub use crate::ui::tooltip::format_tooltip;
pub use crate::ui::tray::{open_sound_settings, open_volume_mixer, TrayWrapper};
pub use crate::ui::wheel::{calc_step as calc_wheel_step, WheelState};

// Re-export dialog helpers that previously lived here
pub use crate::platform::dialog::{prompt_custom_limit, show_error_invalid_custom};

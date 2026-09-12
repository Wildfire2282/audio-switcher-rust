//! Menu action dispatch — maps `muda` IDs to typed actions.

use crate::platform::hotkey::HotkeyAction;

/// Typed menu action parsed from a `MenuEvent` ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAction {
    /// Switch to device `id`.
    Device(String),
    /// Switch to input (capture) device `id`.
    InputDevice(String),
    /// Toggle mute.
    Mute,
    /// Toggle volume-limit enabled.
    VolEnabled,
    /// Set limit to `u32` percent.
    VolLimit(u32),
    /// Re-enumerate devices (manual refresh fallback).
    Refresh,
    /// Open volume mixer.
    OpenMixer,
    /// Open sound settings.
    OpenSound,
    /// Toggle autostart.
    Autostart,
    /// Bind/unbind the default combination for `action`.
    HotkeyToggle(HotkeyAction),
    /// Follow the system language.
    LangSystem,
    /// Switch language to Chinese.
    LangZh,
    /// Switch language to English.
    LangEn,
    /// Open about URL.
    About,
    /// Exit process.
    Exit,
    /// Unknown ID — warned and ignored by the caller.
    Unknown(String),
}

impl MenuAction {
    /// Parse a menu ID into a typed action.
    ///
    /// IDs are produced by [`crate::ui::menu`]; device IDs keep the
    /// [`crate::ui::menu::DEVICE_PREFIX`] prefix and `vol_N` presets parse
    /// through [`crate::ui::menu::id::parse_vol_preset`].
    #[must_use]
    pub fn from_id(id: &str) -> Self {
        use crate::ui::menu::{DEVICE_PREFIX, INPUT_PREFIX, id as menu_id};
        if let Some(dev) = id.strip_prefix(DEVICE_PREFIX) {
            if dev.is_empty() || dev.contains('\0') {
                return Self::Unknown(id.to_string());
            }
            return Self::Device(dev.to_string());
        }
        if let Some(dev) = id.strip_prefix(INPUT_PREFIX) {
            if dev.is_empty() || dev.contains('\0') {
                return Self::Unknown(id.to_string());
            }
            return Self::InputDevice(dev.to_string());
        }
        if let Some(preset) = menu_id::parse_vol_preset(id) {
            return Self::VolLimit(preset);
        }
        if let Some(action) = menu_id::parse_hotkey(id) {
            return Self::HotkeyToggle(action);
        }
        match id {
            menu_id::REFRESH => Self::Refresh,
            menu_id::MUTE => Self::Mute,
            menu_id::VOL_ENABLED => Self::VolEnabled,
            menu_id::OPEN_MIXER => Self::OpenMixer,
            menu_id::OPEN_SOUND => Self::OpenSound,
            menu_id::AUTOSTART => Self::Autostart,
            menu_id::LANG_SYSTEM => Self::LangSystem,
            menu_id::LANG_ZH => Self::LangZh,
            menu_id::LANG_EN => Self::LangEn,
            menu_id::ABOUT => Self::About,
            menu_id::EXIT => Self::Exit,
            other => Self::Unknown(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_device() {
        assert_eq!(
            MenuAction::from_id("device_abc"),
            MenuAction::Device("abc".into())
        );
        assert_eq!(MenuAction::from_id("mute"), MenuAction::Mute);
        assert_eq!(MenuAction::from_id("refresh"), MenuAction::Refresh);
        assert_eq!(MenuAction::from_id("vol_25"), MenuAction::VolLimit(25));
        assert_eq!(MenuAction::from_id("vol_50"), MenuAction::VolLimit(50));
        assert_eq!(MenuAction::from_id("vol_75"), MenuAction::VolLimit(75));
        assert_eq!(MenuAction::from_id("lang_system"), MenuAction::LangSystem);
        assert!(matches!(
            MenuAction::from_id("unknown"),
            MenuAction::Unknown(_)
        ));
    }

    #[test]
    fn parse_hotkey_toggles() {
        for action in HotkeyAction::ALL {
            let id = crate::ui::menu::id::hotkey(action);
            assert_eq!(MenuAction::from_id(&id), MenuAction::HotkeyToggle(action));
        }
        // The submenu id and unknown suffixes stay Unknown.
        for id in ["hotkeys", "hotkey_", "hotkey_bogus", "hotkey_volume"] {
            assert!(
                matches!(MenuAction::from_id(id), MenuAction::Unknown(_)),
                "{id}"
            );
        }
    }

    #[test]
    fn parse_input_device() {
        assert_eq!(
            MenuAction::from_id("input_m1"),
            MenuAction::InputDevice("m1".into())
        );
        assert_eq!(
            MenuAction::from_id("device_abc"),
            MenuAction::Device("abc".into())
        );
        assert!(matches!(
            MenuAction::from_id("input_"),
            MenuAction::Unknown(_)
        ));
    }

    #[test]
    fn parse_vol_preset_only_accepts_menu_presets() {
        // The menu only emits 25/50/75 — anything else stays Unknown.
        assert!(matches!(
            MenuAction::from_id("vol_30"),
            MenuAction::Unknown(_)
        ));
        assert!(matches!(
            MenuAction::from_id("vol_0"),
            MenuAction::Unknown(_)
        ));
        assert!(matches!(
            MenuAction::from_id("vol_101"),
            MenuAction::Unknown(_)
        ));
        assert!(matches!(
            MenuAction::from_id("vol_x"),
            MenuAction::Unknown(_)
        ));
    }

    #[test]
    fn unknown_ids_stay_unknown_for_warn_path() {
        // The App dispatch warns (debug_assert + tracing::warn) and ignores
        // these; parsing must never coerce them into a real action.
        for id in ["bogus", "", "vol_", "device_", "DEVICE_abc", "About"] {
            assert!(
                matches!(MenuAction::from_id(id), MenuAction::Unknown(_)),
                "{id} must parse as Unknown"
            );
        }
    }
}

//! Menu action dispatch — maps `muda` IDs to typed actions.

/// Typed menu action parsed from a `MenuEvent` ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAction {
    /// Switch to device `id`.
    Device(String),
    /// Toggle mute.
    Mute,
    /// Toggle volume-limit enabled.
    VolEnabled,
    /// Set limit to `u32` percent.
    VolLimit(u32),
    /// Toggle wheel acceleration.
    WheelAccel,
    /// Open volume mixer.
    OpenMixer,
    /// Open sound settings.
    OpenSound,
    /// Toggle autostart.
    Autostart,
    /// Switch language to Chinese.
    LangZh,
    /// Switch language to English.
    LangEn,
    /// Open about URL.
    About,
    /// Exit process.
    Exit,
    /// Unknown ID — ignored.
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
        use crate::ui::menu::{DEVICE_PREFIX, id as menu_id};
        if let Some(dev) = id.strip_prefix(DEVICE_PREFIX) {
            if dev.is_empty() || dev.contains('\0') {
                return Self::Unknown(id.to_string());
            }
            return Self::Device(dev.to_string());
        }
        if let Some(preset) = menu_id::parse_vol_preset(id) {
            return Self::VolLimit(preset);
        }
        match id {
            menu_id::MUTE => Self::Mute,
            menu_id::VOL_ENABLED => Self::VolEnabled,
            menu_id::WHEEL_ACCEL => Self::WheelAccel,
            menu_id::OPEN_MIXER => Self::OpenMixer,
            menu_id::OPEN_SOUND => Self::OpenSound,
            menu_id::AUTOSTART => Self::Autostart,
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
        assert_eq!(MenuAction::from_id("device_abc"), MenuAction::Device("abc".into()));
        assert_eq!(MenuAction::from_id("mute"), MenuAction::Mute);
        assert_eq!(MenuAction::from_id("vol_25"), MenuAction::VolLimit(25));
        assert_eq!(MenuAction::from_id("vol_50"), MenuAction::VolLimit(50));
        assert_eq!(MenuAction::from_id("vol_75"), MenuAction::VolLimit(75));
        assert!(matches!(MenuAction::from_id("unknown"), MenuAction::Unknown(_)));
    }

    #[test]
    fn parse_vol_preset_only_accepts_menu_presets() {
        // The menu only emits 25/50/75 — anything else stays Unknown.
        assert!(matches!(MenuAction::from_id("vol_30"), MenuAction::Unknown(_)));
        assert!(matches!(MenuAction::from_id("vol_0"), MenuAction::Unknown(_)));
        assert!(matches!(MenuAction::from_id("vol_101"), MenuAction::Unknown(_)));
        assert!(matches!(MenuAction::from_id("vol_x"), MenuAction::Unknown(_)));
    }
}

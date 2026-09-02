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
    #[must_use]
    pub fn from_id(id: &str) -> Self {
        if let Some(dev) = id.strip_prefix("device_") {
            if dev.is_empty() || dev.contains('\0') {
                return Self::Unknown(id.to_string());
            }
            return Self::Device(dev.to_string());
        }
        match id {
            "mute" => Self::Mute,
            "vol_enabled" => Self::VolEnabled,
            "vol_25" => Self::VolLimit(25),
            "vol_50" => Self::VolLimit(50),
            "vol_75" => Self::VolLimit(75),
            "wheel_accel" => Self::WheelAccel,
            "open_mixer" => Self::OpenMixer,
            "open_sound" => Self::OpenSound,
            "autostart" => Self::Autostart,
            "lang_zh" => Self::LangZh,
            "lang_en" => Self::LangEn,
            "about" => Self::About,
            "exit" => Self::Exit,
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
}

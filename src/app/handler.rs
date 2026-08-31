#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAction {
    Device(String),
    Mute,
    VolEnabled,
    VolLimit(u32),
    VolCustom,
    WheelAccel,
    OpenMixer,
    OpenSound,
    Autostart,
    LangZh,
    LangEn,
    About,
    Exit,
    Unknown(String),
}

impl MenuAction {
    pub fn from_id(id: &str) -> Self {
        if let Some(dev) = id.strip_prefix("device_") {
            return Self::Device(dev.to_string());
        }
        match id {
            "mute" => Self::Mute,
            "vol_enabled" => Self::VolEnabled,
            "vol_25" => Self::VolLimit(25),
            "vol_50" => Self::VolLimit(50),
            "vol_custom" => Self::VolCustom,
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
        assert!(matches!(MenuAction::from_id("unknown"), MenuAction::Unknown(_)));
    }
}

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const CURRENT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default = "default_lang")]
    pub lang: String,
    #[serde(default = "default_volume_limit_enabled")]
    pub volume_limit_enabled: bool,
    #[serde(default = "default_volume_limit")]
    pub volume_limit: u32,
    #[serde(default = "default_wheel_accel")]
    pub wheel_acceleration: bool,
    #[serde(default = "default_verbose_log")]
    pub verbose_log: bool,
    #[serde(default = "default_autostart")]
    pub autostart: bool,
}

fn default_version() -> u32 {
    CURRENT_VERSION
}
fn default_lang() -> String {
    "zh".to_string()
}
fn default_volume_limit_enabled() -> bool {
    true
}
fn default_volume_limit() -> u32 {
    25
}
fn default_wheel_accel() -> bool {
    true
}
fn default_verbose_log() -> bool {
    false
}
fn default_autostart() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            lang: default_lang(),
            volume_limit_enabled: default_volume_limit_enabled(),
            volume_limit: default_volume_limit(),
            wheel_acceleration: default_wheel_accel(),
            verbose_log: default_verbose_log(),
            autostart: default_autostart(),
        }
    }
}

impl AppConfig {
    pub fn is_zh(&self) -> bool {
        self.lang == "zh"
    }

    pub fn config_path() -> PathBuf {
        if let Ok(appdata) = std::env::var("APPDATA") {
            PathBuf::from(appdata)
                .join("AudioSwitcher")
                .join("config.json")
        } else if let Ok(home) = std::env::var("USERPROFILE") {
            PathBuf::from(home)
                .join("AppData")
                .join("Roaming")
                .join("AudioSwitcher")
                .join("config.json")
        } else {
            PathBuf::from("config.json")
        }
    }

    pub fn config_path_for(dir: &Path) -> PathBuf {
        dir.join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        Self::load_from(&path)
    }

    pub fn load_from(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(content) => match serde_json::from_str::<AppConfig>(&content) {
                Ok(mut cfg) => {
                    // migration / validation
                    if cfg.version != CURRENT_VERSION {
                        cfg.version = CURRENT_VERSION;
                    }
                    if cfg.lang != "zh" && cfg.lang != "en" {
                        cfg.lang = default_lang();
                    }
                    if !(1..=100).contains(&cfg.volume_limit) {
                        cfg.volume_limit = default_volume_limit();
                    }
                    cfg
                }
                Err(_) => {
                    let def = AppConfig::default();
                    let _ = def.save_to(path);
                    def
                }
            },
            Err(_) => {
                let def = AppConfig::default();
                // try to save defaults, ignore errors
                let _ = def.save_to(path);
                def
            }
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::config_path();
        self.save_to(&path)
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self).unwrap();
        std::fs::write(path, json)
    }

    /// Validate custom threshold string, return Ok(value) or Err(message key)
    pub fn validate_custom_limit(s: &str) -> Result<u32, &'static str> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err("invalid");
        }
        match trimmed.parse::<i32>() {
            Ok(v) if (1..=100).contains(&v) => Ok(v as u32),
            _ => Err("invalid"),
        }
    }
}

/// Clamp volume according to config
pub fn clamp_volume(volume: u32, cfg: &AppConfig) -> u32 {
    if cfg.volume_limit_enabled {
        volume.min(cfg.volume_limit)
    } else {
        volume
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn default_values() {
        let c = AppConfig::default();
        assert_eq!(c.lang, "zh");
        assert!(c.volume_limit_enabled);
        assert_eq!(c.volume_limit, 25);
        assert!(c.wheel_acceleration);
        assert!(!c.verbose_log);
        assert!(c.autostart);
        assert_eq!(c.version, 1);
    }

    #[test]
    fn clamp_enabled() {
        let mut cfg = AppConfig::default();
        cfg.volume_limit_enabled = true;
        cfg.volume_limit = 25;
        assert_eq!(clamp_volume(30, &cfg), 25);
        assert_eq!(clamp_volume(20, &cfg), 20);
    }

    #[test]
    fn clamp_disabled() {
        let mut cfg = AppConfig::default();
        cfg.volume_limit_enabled = false;
        assert_eq!(clamp_volume(80, &cfg), 80);
    }

    #[test]
    fn validate_custom() {
        assert_eq!(AppConfig::validate_custom_limit("50").unwrap(), 50);
        assert_eq!(AppConfig::validate_custom_limit("  100 ").unwrap(), 100);
        assert!(AppConfig::validate_custom_limit("0").is_err());
        assert!(AppConfig::validate_custom_limit("101").is_err());
        assert!(AppConfig::validate_custom_limit("abc").is_err());
        assert!(AppConfig::validate_custom_limit("").is_err());
    }

    #[test]
    fn persistence_with_tempfile() {
        let dir = tempdir().unwrap();
        let path = AppConfig::config_path_for(dir.path());
        let cfg = AppConfig {
            lang: "en".to_string(),
            volume_limit: 50,
            ..Default::default()
        };
        cfg.save_to(&path).unwrap();
        let loaded = AppConfig::load_from(&path);
        assert_eq!(loaded.lang, "en");
        assert_eq!(loaded.volume_limit, 50);
    }

    #[test]
    fn corrupted_fallback() {
        let dir = tempdir().unwrap();
        let path = AppConfig::config_path_for(dir.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "not json").unwrap();
        let loaded = AppConfig::load_from(&path);
        assert_eq!(loaded, AppConfig::default());
    }

    #[test]
    fn migration_version() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, r#"{"lang":"zh","version":0}"#).unwrap();
        let loaded = AppConfig::load_from(&path);
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.lang, "zh");
    }
}

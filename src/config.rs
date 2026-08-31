//! Application configuration persistence.
//!
//! `AppConfig` is stored as JSON at `%APPDATA%\AudioSwitcher\config.json`.
//! All public fields have `serde(default)` so older files stay compatible.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::LazyLock;

const CURRENT_VERSION: u32 = 1;

/// Cached config path — computed once per process.
static CONFIG_PATH_CACHE: LazyLock<PathBuf> = LazyLock::new(|| {
    let try_dir = |p: PathBuf| {
        if p.is_absolute() {
            Some(p.join("AudioSwitcher").join("config.json"))
        } else {
            None
        }
    };
    if let Ok(appdata) = std::env::var("APPDATA") {
        if let Some(p) = try_dir(PathBuf::from(appdata)) {
            return p;
        }
    }
    if let Ok(home) = std::env::var("USERPROFILE") {
        if let Some(p) = try_dir(PathBuf::from(home).join("AppData").join("Roaming")) {
            return p;
        }
    }
    // Fallback to temp_dir to avoid writing to attacker-controlled CWD (e.g. System32).
    if let Some(p) = try_dir(std::env::temp_dir()) {
        return p;
    } else {
        PathBuf::from("config.json")
    }
});

// ---------------------------------------------------------------------------
// Lang
// ---------------------------------------------------------------------------

/// UI language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    /// Chinese.
    #[default]
    Zh,
    /// English.
    En,
}

impl Lang {
    /// String representation as stored in JSON / config.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Zh => "zh",
            Self::En => "en",
        }
    }

    /// Whether this is Chinese.
    #[must_use]
    pub fn is_zh(self) -> bool {
        self == Self::Zh
    }
}

impl std::fmt::Display for Lang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Lang {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "zh" | "chinese" | "cn" => Ok(Self::Zh),
            "en" | "english" => Ok(Self::En),
            _ => Err("unknown language"),
        }
    }
}

// serde: store as lowercase string, tolerant to unknown values (fallback Zh)
fn deserialize_lang<'de, D>(de: D) -> Result<Lang, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(de)?;
    Ok(s.parse::<Lang>().unwrap_or(Lang::Zh))
}

fn serialize_lang<S>(lang: &Lang, ser: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    ser.serialize_str(lang.as_str())
}

// Lang uses deserialize_lang/serialize_lang via AppConfig field attributes;
// no separate Serialize/Deserialize impl to avoid duplication (see review).
impl Serialize for Lang {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_lang(self, ser)
    }
}

impl<'de> Deserialize<'de> for Lang {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_lang(de)
    }
}

fn deserialize_volume_limit<'de, D>(de: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error as DeError;
    let v = serde_json::Value::deserialize(de)?;
    match v {
        serde_json::Value::Number(n) => n
            .as_u64()
            .and_then(|x| u32::try_from(x).ok())
            .ok_or_else(|| DeError::custom("invalid volume_limit")),
        serde_json::Value::String(s) => {
            s.trim().parse::<u32>().map_err(|_| DeError::custom("invalid volume_limit"))
        }
        _ => Err(DeError::custom("invalid volume_limit")),
    }
}

// ---------------------------------------------------------------------------
// AppConfig
// ---------------------------------------------------------------------------

/// Persisted application configuration.
///
/// # Examples
///
/// ```
/// use audio_switcher_rust::config::{AppConfig, Lang};
/// let cfg = AppConfig::default();
/// assert_eq!(cfg.lang, Lang::Zh);
/// assert_eq!(cfg.volume_limit, 25);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    /// Config schema version for migrations.
    #[serde(default = "default_version")]
    pub version: u32,
    /// UI language.
    #[serde(
        default = "default_lang",
        deserialize_with = "deserialize_lang",
        serialize_with = "serialize_lang"
    )]
    pub lang: Lang,
    /// Whether volume limiting is enabled.
    #[serde(default = "default_volume_limit_enabled")]
    pub volume_limit_enabled: bool,
    /// Maximum volume percent when limiting is enabled (1..=100).
    #[serde(
        default = "default_volume_limit",
        deserialize_with = "deserialize_volume_limit",
        alias = "volumeLimit",
        alias = "VolumeLimit"
    )]
    pub volume_limit: u32,
    /// Whether wheel acceleration (fast scroll → larger steps) is enabled.
    #[serde(default = "default_wheel_accel")]
    pub wheel_acceleration: bool,
    /// Whether to register for auto-launch at login.
    #[serde(default = "default_autostart")]
    pub autostart: bool,
}

fn default_version() -> u32 {
    CURRENT_VERSION
}
fn default_lang() -> Lang {
    Lang::Zh
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
            autostart: default_autostart(),
        }
    }
}

impl AppConfig {
    /// Returns the cached config file path.
    #[must_use]
    pub fn config_path() -> PathBuf {
        (*CONFIG_PATH_CACHE).clone()
    }

    /// Test helper: config path inside a temp dir.
    #[cfg(test)]
    #[must_use]
    pub fn config_path_for(dir: &Path) -> PathBuf {
        dir.join("config.json")
    }

    /// Load config from the standard location, falling back to defaults.
    #[must_use]
    pub fn load() -> Self {
        let path = Self::config_path();
        Self::load_from(&path)
    }

    /// Load config from an explicit path with validation and migration.
    #[must_use]
    pub fn load_from(path: &Path) -> Self {
        match std::fs::read(path) {
            Ok(bytes) => match serde_json::from_slice::<Self>(&bytes) {
                Ok(mut cfg) => {
                    // Version migration scaffold.
                    match cfg.version {
                        CURRENT_VERSION => {}
                        0 => {
                            cfg.version = CURRENT_VERSION;
                        }
                        _ => {
                            // Unknown future version — keep fields but bump version.
                            cfg.version = CURRENT_VERSION;
                        }
                    }
                    if !(1..=100).contains(&cfg.volume_limit) {
                        cfg.volume_limit = default_volume_limit();
                    }
                    cfg
                }
                Err(e) => {
                    // Try tolerant recovery: salvage valid fields via Value merge.
                    if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                        if let serde_json::Value::Object(mut map) = val {
                            // Ensure volume_limit is salvageable even if string-typed.
                            if let Some(v) = map.get("volume_limit").cloned() {
                                if let serde_json::Value::String(s) = v {
                                    if let Ok(n) = s.trim().parse::<u32>() {
                                        if (1..=100).contains(&n) {
                                            map.insert(
                                                "volume_limit".into(),
                                                serde_json::Value::Number(n.into()),
                                            );
                                        }
                                    }
                                }
                            }
                            if let Ok(mut cfg) = serde_json::from_value::<Self>(
                                serde_json::Value::Object(map.clone()),
                            ) {
                                if !(1..=100).contains(&cfg.volume_limit) {
                                    cfg.volume_limit = default_volume_limit();
                                }
                                cfg.version = CURRENT_VERSION;
                                let _ = cfg.save_to(path);
                                return cfg;
                            }
                        }
                    }
                    // Corrupted file — back up original before overwriting.
                    let backup = {
                        let nanos = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_nanos())
                            .unwrap_or(0);
                        let pid = std::process::id();
                        path.with_file_name(format!(
                            "{}.bak.{}-{}",
                            path.file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| "config.json".into()),
                            nanos,
                            pid
                        ))
                    };
                    let _ = std::fs::write(&backup, &bytes);
                    let _ = e;
                    let def = Self::default();
                    let _ = def.save_to(path);
                    def
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let def = Self::default();
                let _ = def.save_to(path);
                def
            }
            Err(_) => {
                // Transient IO error (e.g. permission) — don't clobber file, return in-memory default.
                Self::default()
            }
        }
    }

    /// Non-blocking save; returns a handle that can be joined in tests.
    ///
    /// Fire-and-forget callers may drop the handle, but the write may be lost
    /// if the process exits before the thread completes. Prefer joining the
    /// handle at exit or using [`Self::save_to`] synchronously for critical saves.
    pub fn save(&self) -> std::thread::JoinHandle<std::io::Result<()>> {
        let cfg = self.clone();
        let path = Self::config_path();
        std::thread::spawn(move || cfg.save_to(&path))
    }

    /// Synchronous atomic save: write to a unique temporary file alongside the
    /// target then rename. The unique suffix avoids races between concurrent
    /// `save()` callers.
    ///
    /// # Errors
    ///
    /// Returns `io::Error` if directory creation, write, or rename fails.
    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // SAFETY: AppConfig is always serializable.
        let json = serde_json::to_string_pretty(self).expect("AppConfig serialization never fails");
        let tmp_path = {
            static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let file_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "config.json".into());
            let suffix = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let pid = std::process::id();
            path.with_file_name(format!("{file_name}.tmp.{pid}-{nanos}-{suffix}"))
        };
        std::fs::write(&tmp_path, json.as_bytes())?;
        // On Windows rename uses MoveFileExW(REPLACE_EXISTING) and atomically replaces.
        if let Err(e) = std::fs::rename(&tmp_path, path) {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(e);
        }
        Ok(())
    }

    /// Validate a custom threshold string.
    ///
    /// Returns `Ok(value)` for integers `1..=100`, `Err("invalid")` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use audio_switcher_rust::config::AppConfig;
    /// assert_eq!(AppConfig::validate_custom_limit("50").unwrap(), 50);
    /// assert!(AppConfig::validate_custom_limit("0").is_err());
    /// ```
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

/// Clamp `volume` according to `cfg`.
/// When limiting is disabled the result is still capped to 100 to preserve invariant.
#[must_use]
pub fn clamp_volume(volume: u32, cfg: &AppConfig) -> u32 {
    if cfg.volume_limit_enabled {
        volume.min(cfg.volume_limit)
    } else {
        volume.min(100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn default_values() {
        let c = AppConfig::default();
        assert_eq!(c.lang, Lang::Zh);
        assert_eq!(c.lang.as_str(), "zh");
        assert!(c.volume_limit_enabled);
        assert_eq!(c.volume_limit, 25);
        assert!(c.wheel_acceleration);
        assert!(c.autostart);
        assert_eq!(c.version, 1);
    }

    #[test]
    fn lang_roundtrip() {
        assert_eq!("zh".parse::<Lang>().unwrap(), Lang::Zh);
        assert_eq!("en".parse::<Lang>().unwrap(), Lang::En);
        assert_eq!(Lang::Zh.to_string(), "zh");
        assert_eq!(Lang::En.to_string(), "en");
    }

    #[test]
    fn clamp_enabled() {
        let cfg = AppConfig { volume_limit_enabled: true, volume_limit: 25, ..Default::default() };
        assert_eq!(clamp_volume(30, &cfg), 25);
        assert_eq!(clamp_volume(20, &cfg), 20);
    }

    #[test]
    fn clamp_disabled() {
        let cfg = AppConfig { volume_limit_enabled: false, ..Default::default() };
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
        let cfg = AppConfig { lang: Lang::En, volume_limit: 50, ..Default::default() };
        cfg.save_to(&path).unwrap();
        let loaded = AppConfig::load_from(&path);
        assert_eq!(loaded.lang, Lang::En);
        assert_eq!(loaded.volume_limit, 50);
    }

    #[test]
    fn migration_version() {
        let dir = tempdir().unwrap();
        let path = AppConfig::config_path_for(dir.path());
        let old = r#"{"version":0,"lang":"zh","volume_limit_enabled":true,"volume_limit":25,"wheel_acceleration":true,"autostart":true}"#;
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, old).unwrap();
        let loaded = AppConfig::load_from(&path);
        assert_eq!(loaded.version, 1);
    }

    #[test]
    fn corrupted_fallback() {
        let dir = tempdir().unwrap();
        let path = AppConfig::config_path_for(dir.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "not json").unwrap();
        let loaded = AppConfig::load_from(&path);
        assert_eq!(loaded, AppConfig::default());
        assert!(path.exists());
    }

    #[test]
    fn unknown_lang_fallback() {
        let dir = tempdir().unwrap();
        let path = AppConfig::config_path_for(dir.path());
        let raw = r#"{"version":1,"lang":"fr","volume_limit_enabled":true,"volume_limit":25,"wheel_acceleration":true,"autostart":true}"#;
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, raw).unwrap();
        let loaded = AppConfig::load_from(&path);
        // unknown lang maps to Zh via tolerant deserialize
        assert_eq!(loaded.lang, Lang::Zh);
    }
}

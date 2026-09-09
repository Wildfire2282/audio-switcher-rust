//! Application configuration persistence.
//!
//! `AppConfig` is stored as JSON at `%APPDATA%\audio-switcher\config.json`
//! (kebab-case dir from `TOOL_ID`). Lookup chain: `%APPDATA%` → then
//! `%LOCALAPPDATA%` → then temp (degraded: in-memory load still works, every
//! save logs a warning). The `./config.json` fallback is banned (`Program
//! Files` is not writable). Unknown fields are rejected so config typos fail
//! loudly (backup + reset to defaults).

use serde::{Deserialize, Deserializer, Serialize};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::LazyLock;

/// Current config schema version. v2 migrates the v1 `Zh` default to
/// `System` (v1 could not distinguish an explicit `zh` choice from the old
/// default, so explicit `zh` users re-pick once).
const CURRENT_VERSION: u32 = 2;

/// Legacy (v1, PascalCase) config filename for one-time import.
const LEGACY_DIR_NAME: &str = "AudioSwitcher";

/// Cached config path — computed once per process.
static CONFIG_PATH_CACHE: LazyLock<(PathBuf, bool)> = LazyLock::new(resolve_config_path);

// ---------------------------------------------------------------------------
// Lang
// ---------------------------------------------------------------------------

/// UI language. `System` (the default) follows the OS locale once at
/// startup; live re-resolution is deferred (see SPEC §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    /// Follow the system locale.
    #[default]
    System,
    /// Chinese (Simplified).
    Zh,
    /// English.
    En,
}

impl Lang {
    /// String representation as stored in JSON / config.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Zh => "zh",
            Self::En => "en",
        }
    }

    /// Whether this is Chinese.
    #[must_use]
    pub fn is_zh(self) -> bool {
        self == Self::Zh
    }

    /// Map a locale name (`"zh-CN"`, `"en-US"`) to a language. Pure and
    /// branch-tested. Traditional-Chinese locales fall back to English
    /// (untranslated, recorded in SPEC); everything else is English.
    #[must_use]
    pub fn for_locale_name(name: &str) -> Self {
        let lower = name.to_ascii_lowercase();
        if lower.starts_with("zh") {
            let traditional = lower.contains("hant")
                || ["-hk", "-tw", "-mo", "_hk", "_tw", "_mo"]
                    .iter()
                    .any(|s| lower.contains(s));
            if traditional {
                return Self::En;
            }
            return Self::Zh;
        }
        Self::En
    }

    /// Resolve the system locale once. Read failure falls back to English
    /// with a warning (never "failure means Chinese").
    #[must_use]
    pub fn system() -> Self {
        let name = crate::platform::locale::system_locale_name();
        if name.is_empty() {
            tracing::warn!("system locale unreadable, falling back to English");
            return Self::En;
        }
        Self::for_locale_name(&name)
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
            "system" | "auto" | "follow" => Ok(Self::System),
            "zh" | "chinese" | "cn" => Ok(Self::Zh),
            "en" | "english" => Ok(Self::En),
            _ => Err("unknown language"),
        }
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
        serde_json::Value::String(s) => s
            .trim()
            .parse::<u32>()
            .map_err(|_| DeError::custom("invalid volume_limit")),
        _ => Err(DeError::custom("invalid volume_limit")),
    }
}

// ---------------------------------------------------------------------------
// AppConfig
// ---------------------------------------------------------------------------

/// Persisted application configuration.
///
/// Unknown fields are rejected (`deny_unknown_fields`): a typo must reset
/// loudly (backup + defaults), never be silently ignored.
///
/// # Examples
///
/// ```
/// use audio_switcher::config::{AppConfig, Lang};
/// let cfg = AppConfig::default();
/// assert_eq!(cfg.lang, Lang::System);
/// assert_eq!(cfg.volume_limit, 25);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AppConfig {
    /// Config schema version for migrations.
    #[serde(default = "default_version")]
    pub version: u32,
    /// UI language mode.
    #[serde(default = "default_lang")]
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
    /// Whether to register for auto-launch at login.
    #[serde(default = "default_autostart")]
    pub autostart: bool,
}

fn default_version() -> u32 {
    CURRENT_VERSION
}
fn default_lang() -> Lang {
    Lang::System
}
fn default_volume_limit_enabled() -> bool {
    true
}
fn default_volume_limit() -> u32 {
    25
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
            autostart: default_autostart(),
        }
    }
}

/// Resolve `(path, degraded)` from explicit roots. Pure for tests; the live
/// [`resolve_config_path`] reads the environment.
fn resolve_for(appdata: Option<&str>, localappdata: Option<&str>, tmp: &Path) -> (PathBuf, bool) {
    let under = |root: &str| {
        let dir = PathBuf::from(root);
        if dir.is_absolute() {
            Some(dir.join(crate::TOOL_ID).join("config.json"))
        } else {
            None
        }
    };
    if let Some(path) = appdata.and_then(under) {
        return (path, false);
    }
    if let Some(path) = localappdata.and_then(under) {
        return (path, false);
    }
    (tmp.join(crate::TOOL_ID).join("config.json"), true)
}

fn resolve_config_path() -> (PathBuf, bool) {
    resolve_for(
        std::env::var("APPDATA").ok().as_deref(),
        std::env::var("LOCALAPPDATA").ok().as_deref(),
        &std::env::temp_dir(),
    )
}

/// Legacy v1 path (`%APPDATA%\AudioSwitcher\config.json`) for one-time import.
fn legacy_config_path() -> Option<PathBuf> {
    let appdata = std::env::var("APPDATA").ok()?;
    let dir = PathBuf::from(appdata);
    if !dir.is_absolute() {
        return None;
    }
    Some(dir.join(LEGACY_DIR_NAME).join("config.json"))
}

/// Copy the legacy file to `new_path` when `new_path` is missing. Pure over
/// explicit paths so tests can isolate it from the real `%APPDATA%`.
fn import_legacy_file(new_path: &Path, legacy_path: &Path) -> bool {
    if new_path.exists() || !legacy_path.exists() {
        return false;
    }
    let Ok(bytes) = std::fs::read(legacy_path) else {
        return false;
    };
    let cfg = serde_json::from_slice::<AppConfig>(&bytes)
        .map(AppConfig::migrate)
        .unwrap_or_default();
    cfg.save_to(new_path).is_ok()
}

impl AppConfig {
    /// Returns the cached config file path.
    #[must_use]
    pub fn config_path() -> PathBuf {
        CONFIG_PATH_CACHE.0.clone()
    }

    /// Whether the resolved path is the degraded temp fallback (saves warn).
    #[must_use]
    pub fn config_degraded() -> bool {
        CONFIG_PATH_CACHE.1
    }

    /// Effective UI language: `System` resolves the OS locale once.
    #[must_use]
    pub fn effective_lang(&self) -> Lang {
        match self.lang {
            Lang::System => Lang::system(),
            other => other,
        }
    }

    /// Test helper: config path inside a temp dir.
    #[cfg(test)]
    #[must_use]
    pub fn config_path_for(dir: &Path) -> PathBuf {
        dir.join("config.json")
    }

    /// Load config from the standard location, falling back to defaults.
    /// Imports the legacy PascalCase file once when the new path is missing.
    #[must_use]
    pub fn load() -> Self {
        let path = Self::config_path();
        if !path.exists() {
            if let Some(legacy) = legacy_config_path() {
                if import_legacy_file(&path, &legacy) {
                    tracing::debug!("imported legacy config");
                }
            }
        }
        Self::load_from(&path)
    }

    /// Load config from an explicit path with validation and migration.
    #[must_use]
    pub fn load_from(path: &Path) -> Self {
        match std::fs::read(path) {
            Ok(bytes) => Self::load_from_bytes(&bytes, path),
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

    /// Parse `bytes` read from `path`, migrating and validating. Unknown
    /// fields or corrupt JSON back the file up and reset to defaults (loud,
    /// never silent).
    fn load_from_bytes(bytes: &[u8], path: &Path) -> Self {
        match serde_json::from_slice::<Self>(bytes) {
            Ok(cfg) => Self::migrate(cfg),
            Err(e) => {
                tracing::warn!("config parse failed ({e}); backing up and resetting");
                Self::backup_and_reset(bytes, path)
            }
        }
    }

    /// Migrate older schemas: bump the version, clamp the limit, and move
    /// the v1 `Zh` default to `System`.
    fn migrate(mut cfg: Self) -> Self {
        if cfg.version < CURRENT_VERSION && cfg.lang == Lang::Zh {
            cfg.lang = Lang::System;
        }
        cfg.version = CURRENT_VERSION;
        if !(1..=100).contains(&cfg.volume_limit) {
            cfg.volume_limit = default_volume_limit();
        }
        cfg
    }

    /// Back up offending `bytes` next to `path`, overwrite with defaults, and return them.
    fn backup_and_reset(bytes: &[u8], path: &Path) -> Self {
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
        let _ = std::fs::write(&backup, bytes);
        let def = Self::default();
        let _ = def.save_to(path);
        def
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
    /// `save()` callers. Warns when writing to the degraded temp fallback.
    ///
    /// # Errors
    ///
    /// Returns `io::Error` if directory creation, write, or rename fails.
    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if Self::config_degraded() {
            tracing::warn!("saving to degraded temp config path");
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
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
    /// use audio_switcher::config::AppConfig;
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
/// Output is always capped to 100 to preserve invariant,
/// even if `volume_limit` is out of range via direct construction.
#[must_use]
pub fn clamp_volume(volume: u32, cfg: &AppConfig) -> u32 {
    if cfg.volume_limit_enabled {
        volume.min(cfg.volume_limit.min(100))
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
        assert_eq!(c.lang, Lang::System);
        assert_eq!(c.lang.as_str(), "system");
        assert!(c.volume_limit_enabled);
        assert_eq!(c.volume_limit, 25);
        assert!(c.autostart);
        assert_eq!(c.version, CURRENT_VERSION);
    }

    #[test]
    fn lang_roundtrip() {
        assert_eq!("system".parse::<Lang>().unwrap(), Lang::System);
        assert_eq!("zh".parse::<Lang>().unwrap(), Lang::Zh);
        assert_eq!("en".parse::<Lang>().unwrap(), Lang::En);
        assert_eq!(Lang::System.to_string(), "system");
        assert_eq!(Lang::Zh.to_string(), "zh");
        assert_eq!(Lang::En.to_string(), "en");
    }

    #[test]
    fn path_chain_prefers_appdata_then_localappdata_then_temp() {
        // Absolute roots built from the real temp dir: portable across OSes.
        let base = tempdir().unwrap().keep();
        let roaming = base.join("Roaming");
        let local = base.join("Local");
        let tmp = base.join("Tmp");
        let (appdata_path, degraded) = resolve_for(
            Some(roaming.to_string_lossy().as_ref()),
            Some(local.to_string_lossy().as_ref()),
            &tmp,
        );
        assert!(!degraded);
        assert_eq!(
            appdata_path,
            roaming.join("audio-switcher").join("config.json")
        );
        let (local_path, degraded) =
            resolve_for(None, Some(local.to_string_lossy().as_ref()), &tmp);
        assert!(!degraded);
        assert_eq!(local_path, local.join("audio-switcher").join("config.json"));
        let (temp_path, degraded) = resolve_for(None, None, &tmp);
        assert!(degraded);
        assert_eq!(temp_path, tmp.join("audio-switcher").join("config.json"));
        // No ./config.json fallback anywhere in the chain.
        assert!(temp_path.is_absolute());
    }

    #[test]
    fn explicit_modes_survive_effective() {
        let zh = AppConfig {
            lang: Lang::Zh,
            ..Default::default()
        };
        let en = AppConfig {
            lang: Lang::En,
            ..Default::default()
        };
        assert_eq!(zh.effective_lang(), Lang::Zh);
        assert_eq!(en.effective_lang(), Lang::En);
        // System resolves without panicking (value depends on the test machine).
        let sys = AppConfig::default();
        assert!(matches!(
            sys.effective_lang(),
            Lang::Zh | Lang::En | Lang::System
        ));
    }

    #[test]
    fn clamp_enabled() {
        let cfg = AppConfig {
            volume_limit_enabled: true,
            volume_limit: 25,
            ..Default::default()
        };
        assert_eq!(clamp_volume(30, &cfg), 25);
        assert_eq!(clamp_volume(20, &cfg), 20);
    }

    #[test]
    fn clamp_disabled() {
        let cfg = AppConfig {
            volume_limit_enabled: false,
            ..Default::default()
        };
        assert_eq!(clamp_volume(80, &cfg), 80);
        assert_eq!(clamp_volume(100, &cfg), 100);
        // Disabled still preserves the 0..=100 invariant.
        assert_eq!(clamp_volume(120, &cfg), 100);
    }

    #[test]
    fn clamp_enabled_out_of_range_limit_still_caps_at_100() {
        // Defensive: direct struct construction can bypass migrate() clamping.
        let cfg = AppConfig {
            volume_limit_enabled: true,
            volume_limit: 200,
            ..Default::default()
        };
        assert_eq!(clamp_volume(150, &cfg), 100);
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
            lang: Lang::En,
            volume_limit: 50,
            ..Default::default()
        };
        cfg.save_to(&path).unwrap();
        let loaded = AppConfig::load_from(&path);
        assert_eq!(loaded.lang, Lang::En);
        assert_eq!(loaded.volume_limit, 50);
    }

    #[test]
    fn migration_version_bump() {
        let dir = tempdir().unwrap();
        let path = AppConfig::config_path_for(dir.path());
        let old = r#"{"version":0,"lang":"en","volume_limit_enabled":true,"volume_limit":25,"autostart":true}"#;
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, old).unwrap();
        let loaded = AppConfig::load_from(&path);
        assert_eq!(loaded.version, CURRENT_VERSION);
        assert_eq!(loaded.lang, Lang::En);
    }

    #[test]
    fn migration_v1_zh_default_becomes_system() {
        // v1 defaulted to Zh; the implicit choice follows the system now.
        let dir = tempdir().unwrap();
        let path = AppConfig::config_path_for(dir.path());
        let old = r#"{"version":1,"lang":"zh","volume_limit_enabled":true,"volume_limit":25,"autostart":true}"#;
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, old).unwrap();
        let loaded = AppConfig::load_from(&path);
        assert_eq!(loaded.version, CURRENT_VERSION);
        assert_eq!(loaded.lang, Lang::System);
    }

    #[test]
    fn migration_v2_explicit_zh_stays() {
        let dir = tempdir().unwrap();
        let path = AppConfig::config_path_for(dir.path());
        let raw = r#"{"version":2,"lang":"zh","volume_limit_enabled":true,"volume_limit":25,"autostart":true}"#;
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, raw).unwrap();
        let loaded = AppConfig::load_from(&path);
        assert_eq!(loaded.lang, Lang::Zh);
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
    fn unknown_field_rejected_loudly() {
        // deny_unknown_fields: typos reset to defaults with a backup, never
        // silently ignored (replaces the old wheel_acceleration tolerance).
        let dir = tempdir().unwrap();
        let path = AppConfig::config_path_for(dir.path());
        let raw = r#"{"version":2,"lang":"system","volume_limit_enabled":true,"volume_limit":25,"wheel_acceleration":false,"autostart":true}"#;
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, raw).unwrap();
        let loaded = AppConfig::load_from(&path);
        assert_eq!(loaded, AppConfig::default());
        let backup = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .any(|e| e.file_name().to_string_lossy().contains(".bak."));
        assert!(backup, "corrupt config must leave a backup");
    }

    #[test]
    fn unknown_lang_value_rejected_loudly() {
        let dir = tempdir().unwrap();
        let path = AppConfig::config_path_for(dir.path());
        let raw = r#"{"version":2,"lang":"fr","volume_limit_enabled":true,"volume_limit":25,"autostart":true}"#;
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, raw).unwrap();
        let loaded = AppConfig::load_from(&path);
        assert_eq!(loaded, AppConfig::default());
    }

    #[test]
    fn legacy_pascalcase_file_imported_once() {
        let dir = tempdir().unwrap();
        let new_path = dir.path().join("new").join("config.json");
        let legacy_path = dir.path().join("legacy").join("config.json");
        std::fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
        std::fs::write(
            &legacy_path,
            r#"{"version":1,"lang":"en","volume_limit_enabled":true,"volume_limit":50,"autostart":true}"#,
        )
        .unwrap();
        assert!(import_legacy_file(&new_path, &legacy_path));
        let loaded = AppConfig::load_from(&new_path);
        assert_eq!(loaded.lang, Lang::En);
        assert_eq!(loaded.volume_limit, 50);
        // Second run is a no-op (new path exists now).
        assert!(!import_legacy_file(&new_path, &legacy_path));
    }
}

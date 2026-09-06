//! Tray context menu builder.

use muda::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};

use crate::audio::AudioDevice;
use crate::config::{AppConfig, Lang};
use crate::ui::i18n::tr;
use crate::ui::text::{MAX_LABEL_CHARS, truncate_label};

/// Prefix for per-device menu item IDs; the remainder is the WASAPI endpoint ID.
pub const DEVICE_PREFIX: &str = "device_";
/// Prefix for per-input-device menu item IDs; the remainder is the WASAPI endpoint ID.
pub const INPUT_PREFIX: &str = "input_";
/// Volume-limit presets offered in the submenu (percent).
pub const VOLUME_PRESETS: &[u32] = &[25, 50, 75];

/// Menu item IDs shared with [`crate::app::handler::MenuAction::from_id`].
pub mod id {
    /// Toggle global mute.
    pub const MUTE: &str = "mute";
    /// Toggle volume-limit enabled.
    pub const VOL_ENABLED: &str = "vol_enabled";
    /// Open volume mixer.
    pub const OPEN_MIXER: &str = "open_mixer";
    /// Open sound settings.
    pub const OPEN_SOUND: &str = "open_sound";
    /// Toggle autostart.
    pub const AUTOSTART: &str = "autostart";
    /// Switch language to Chinese.
    pub const LANG_ZH: &str = "lang_zh";
    /// Switch language to English.
    pub const LANG_EN: &str = "lang_en";
    /// Open about URL.
    pub const ABOUT: &str = "about";
    /// Exit process.
    pub const EXIT: &str = "exit";

    /// Build the menu ID for a volume-limit preset, e.g. `vol_25`.
    #[must_use]
    pub fn vol_preset(value: u32) -> String {
        format!("vol_{value}")
    }

    /// Parse a `vol_N` ID back into `N`, or `None`.
    ///
    /// Only the presets in [`VOLUME_PRESETS`](super::VOLUME_PRESETS) are accepted
    /// so parsing stays an exact inverse of [`vol_preset`] for IDs this menu
    /// emits; anything else falls through to `Unknown` in the handler.
    #[must_use]
    pub fn parse_vol_preset(id: &str) -> Option<u32> {
        let v = id.strip_prefix("vol_")?.parse::<u32>().ok()?;
        super::VOLUME_PRESETS.contains(&v).then_some(v)
    }
}

/// Handles for the current menu — the `Menu` must be kept alive.
///
/// Item handles are retained so state changes (checks, labels) can be
/// applied in place via [`MenuHandles::sync_state`] instead of rebuilding
/// the whole menu tree.
pub struct MenuHandles {
    /// The root menu attached to the tray.
    pub menu: Menu,
    /// Per-device items keyed by sanitized device id plus the display name
    /// used at build time (rename detection).
    device_items: Vec<(String, String, CheckMenuItem)>,
    /// Per-input-device items, same keying as [`Self::device_items`].
    input_items: Vec<(String, String, CheckMenuItem)>,
    /// Language used for labels at build time (label change detection).
    lang: Lang,
    /// Global mute toggle.
    mute: CheckMenuItem,
    /// Volume-limit enabled toggle.
    vol_enabled: CheckMenuItem,
    /// Volume-limit presets `(percent, item)`.
    vol_items: Vec<(u32, CheckMenuItem)>,
    /// Autostart toggle.
    autostart: CheckMenuItem,
    /// Language switches.
    lang_zh: CheckMenuItem,
    /// Language switches.
    lang_en: CheckMenuItem,
}

impl MenuHandles {
    /// Sanitize a device id the same way [`build_menu`] does.
    fn sanitize_id(id: &str) -> String {
        id.replace(['\0', '\n', '\r'], "_")
    }

    /// Apply state changes in place when the device list is unchanged.
    ///
    /// Returns `false` when a full rebuild is required: device added, removed,
    /// reordered, renamed, or UI language changed (labels are baked at build
    /// time). The caller must then fall back to a full rebuild.
    pub fn sync_state(
        &mut self,
        cfg: &AppConfig,
        devices: &[AudioDevice],
        default_id: Option<&str>,
        inputs: &[AudioDevice],
        default_input_id: Option<&str>,
        muted: bool,
    ) -> bool {
        if self.lang != cfg.lang {
            return false;
        }
        if !sync_entries(&self.device_items, devices, default_id) {
            return false;
        }
        if !sync_entries(&self.input_items, inputs, default_input_id) {
            return false;
        }
        self.mute.set_checked(muted);
        self.vol_enabled.set_checked(cfg.volume_limit_enabled);
        for (preset, item) in &self.vol_items {
            item.set_enabled(cfg.volume_limit_enabled);
            item.set_checked(cfg.volume_limit_enabled && cfg.volume_limit == *preset);
        }
        self.autostart.set_checked(cfg.autostart);
        self.lang_zh.set_checked(cfg.lang == Lang::Zh);
        self.lang_en.set_checked(cfg.lang == Lang::En);
        true
    }
}

/// Build `(key, name, item)` entries for `devices` with `prefix`.
///
/// The key is the sanitized endpoint id used for change detection.
fn check_entries(
    devices: &[AudioDevice],
    prefix: &str,
    default_id: Option<&str>,
) -> Vec<(String, String, CheckMenuItem)> {
    devices
        .iter()
        .map(|dev| {
            let checked = default_id == Some(dev.id.as_str());
            let item = CheckMenuItem::with_id(
                format!("{prefix}{}", MenuHandles::sanitize_id(&dev.id)),
                truncate_label(&dev.name, MAX_LABEL_CHARS),
                true,
                checked,
                None,
            );
            (MenuHandles::sanitize_id(&dev.id), dev.name.clone(), item)
        })
        .collect()
}

/// Refresh `entries` against `devices` in place.
///
/// Returns `false` when a full rebuild is required (count, order, id, or
/// name changed); otherwise updates the checks and returns `true`.
fn sync_entries(
    entries: &[(String, String, CheckMenuItem)],
    devices: &[AudioDevice],
    default_id: Option<&str>,
) -> bool {
    if entries.len() != devices.len() {
        return false;
    }
    for (dev, (key, name, _)) in devices.iter().zip(entries) {
        if MenuHandles::sanitize_id(&dev.id) != *key || dev.name != *name {
            return false;
        }
    }
    for (dev, (_, _, item)) in devices.iter().zip(entries) {
        item.set_checked(default_id == Some(dev.id.as_str()));
    }
    true
}

/// Build the tray menu for `cfg` / `devices`.
///
/// `default_id` is the currently active device; it is shown checked.
#[must_use]
pub fn build_menu(
    cfg: &AppConfig,
    devices: &[AudioDevice],
    default_id: Option<&str>,
    inputs: &[AudioDevice],
    default_input_id: Option<&str>,
    muted: bool,
) -> MenuHandles {
    let lang = cfg.lang;

    let device_items = check_entries(devices, DEVICE_PREFIX, default_id);

    let mute = CheckMenuItem::with_id(id::MUTE, tr("mute", lang), true, muted, None);

    let input_items = check_entries(inputs, INPUT_PREFIX, default_input_id);
    // Disabled section headers; ids avoid the device prefixes so the handler
    // never parses them as device actions even if they were clickable.
    let output_header =
        MenuItem::with_id("outputs_header", tr("output_devices", lang), false, None);
    let input_header = MenuItem::with_id("inputs_header", tr("input_devices", lang), false, None);

    let vol_enabled = CheckMenuItem::with_id(
        id::VOL_ENABLED,
        tr("enabled", lang),
        true,
        cfg.volume_limit_enabled,
        None,
    );
    let vol_items: Vec<CheckMenuItem> = VOLUME_PRESETS
        .iter()
        .map(|preset| {
            CheckMenuItem::with_id(
                id::vol_preset(*preset),
                format!("{preset}%"),
                cfg.volume_limit_enabled,
                cfg.volume_limit == *preset && cfg.volume_limit_enabled,
                None,
            )
        })
        .collect();
    let vol_sep = PredefinedMenuItem::separator();
    let mut vol_refs: Vec<&dyn muda::IsMenuItem> = vec![&vol_enabled, &vol_sep];
    vol_refs.extend(vol_items.iter().map(|item| item as &dyn muda::IsMenuItem));
    let vol_sub =
        Submenu::with_id_and_items("volume_limit", tr("volume_limit", lang), true, &vol_refs)
            .expect("volume_limit submenu");

    let open_mixer = MenuItem::with_id(id::OPEN_MIXER, tr("open_mixer", lang), true, None);
    let open_sound = MenuItem::with_id(id::OPEN_SOUND, tr("open_sound", lang), true, None);
    let autostart =
        CheckMenuItem::with_id(id::AUTOSTART, tr("autostart", lang), true, cfg.autostart, None);
    let lang_zh =
        CheckMenuItem::with_id(id::LANG_ZH, tr("chinese", lang), true, cfg.lang == Lang::Zh, None);
    let lang_en =
        CheckMenuItem::with_id(id::LANG_EN, tr("english", lang), true, cfg.lang == Lang::En, None);
    let lang_sub = Submenu::with_id_and_items("language", "Language", true, &[&lang_zh, &lang_en])
        .expect("language submenu");
    let about = MenuItem::with_id(id::ABOUT, tr("about", lang), true, None);
    let exit = MenuItem::with_id(id::EXIT, tr("exit", lang), true, None);

    let menu = Menu::new();
    if !device_items.is_empty() {
        let _ = menu.append(&output_header);
        for (_, _, item) in &device_items {
            let _ = menu.append(item);
        }
        let _ = menu.append(&PredefinedMenuItem::separator());
    }
    if !input_items.is_empty() {
        let _ = menu.append(&input_header);
        for (_, _, item) in &input_items {
            let _ = menu.append(item);
        }
        let _ = menu.append(&PredefinedMenuItem::separator());
    }
    let _ = menu.append(&mute);
    let _ = menu.append(&vol_sub);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&open_mixer);
    let _ = menu.append(&open_sound);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&autostart);
    let _ = menu.append(&lang_sub);
    let _ = menu.append(&about);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&exit);

    MenuHandles {
        menu,
        device_items,
        input_items,
        lang,
        mute,
        vol_enabled,
        vol_items: VOLUME_PRESETS.iter().zip(vol_items).map(|(p, i)| (*p, i)).collect(),
        autostart,
        lang_zh,
        lang_en,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_devices() -> Vec<AudioDevice> {
        vec![
            AudioDevice { id: "a".into(), name: "Speaker".into() },
            AudioDevice { id: "b".into(), name: "Headset".into() },
        ]
    }

    #[test]
    fn sync_state_updates_checks_in_place() {
        let cfg = AppConfig::default();
        let devices = test_devices();
        let mut handles = build_menu(&cfg, &devices, Some("a"), &[], None, false);
        assert!(handles.sync_state(&cfg, &devices, Some("b"), &[], None, true));
    }

    #[test]
    fn sync_state_rebuilds_on_rename_reorder_and_lang() {
        let cfg = AppConfig::default();
        let devices = test_devices();
        let mut handles = build_menu(&cfg, &devices, Some("a"), &[], None, false);
        // Rename requires rebuild — labels are baked at build time.
        let mut renamed = devices.clone();
        renamed[0].name = "Renamed".into();
        assert!(!handles.sync_state(&cfg, &renamed, Some("a"), &[], None, false));
        // Reorder requires rebuild.
        let mut reordered = devices.clone();
        reordered.reverse();
        assert!(!handles.sync_state(&cfg, &reordered, Some("a"), &[], None, false));
        // Language change requires rebuild — all labels change.
        let mut lang_cfg = cfg.clone();
        lang_cfg.lang = if cfg.lang == Lang::Zh { Lang::En } else { Lang::Zh };
        assert!(!handles.sync_state(&lang_cfg, &devices, Some("a"), &[], None, false));
    }

    #[test]
    fn sync_state_tracks_input_devices() {
        let cfg = AppConfig::default();
        let devices = test_devices();
        let inputs = vec![
            AudioDevice { id: "m1".into(), name: "Mic".into() },
            AudioDevice { id: "m2".into(), name: "Headset Mic".into() },
        ];
        let mut handles = build_menu(&cfg, &devices, Some("a"), &inputs, Some("m1"), false);
        // Default switch applies in place.
        assert!(handles.sync_state(&cfg, &devices, Some("a"), &inputs, Some("m2"), false));
        // Input added requires rebuild.
        let mut grown = inputs.clone();
        grown.push(AudioDevice { id: "m3".into(), name: "Cam Mic".into() });
        assert!(!handles.sync_state(&cfg, &devices, Some("a"), &grown, Some("m2"), false));
    }
}

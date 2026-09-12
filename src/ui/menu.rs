//! Tray context menu builder.
//!
//! Fixed shape (§5.1): grayed `{DisplayName} v{ver}` title → separator →
//! feature group (devices, toggles, system tools) → separator → fixed tail
//! (refresh → autostart → language submenu → about → exit always last,
//! no separators inside the tail).

use muda::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};

use crate::audio::AudioDevice;
use crate::config::{AppConfig, Hotkeys, Lang};
use crate::platform::AutostartState;
use crate::platform::hotkey::HotkeyAction;
use crate::ui::i18n::tr;
use crate::ui::text::{MAX_LABEL_CHARS, truncate_label};

/// Prefix for per-device menu item IDs; the remainder is the WASAPI endpoint ID.
pub const DEVICE_PREFIX: &str = "device_";
/// Prefix for per-input-device menu item IDs; the remainder is the WASAPI endpoint ID.
pub const INPUT_PREFIX: &str = "input_";
/// Volume-limit presets offered in the submenu (percent).
pub const VOLUME_PRESETS: &[u32] = &[25, 50, 75];

/// Menu item IDs shared with [`crate::app::handler::MenuAction::from_id`].
/// 1.0 contract: never rename, never reuse deleted ids; adding is minor.
pub mod id {
    use crate::platform::hotkey::HotkeyAction;

    /// Grayed title (non-clickable, never parsed as an action).
    pub const TITLE: &str = "title";
    /// Manual device-list refresh (sleep-resume/callback-loss fallback).
    pub const REFRESH: &str = "refresh";
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
    /// Hotkeys submenu (one `hotkey_*` child per action).
    pub const HOTKEYS: &str = "hotkeys";
    /// Language submenu (frozen tail id, same contract as the lang_* items).
    pub const LANGUAGE: &str = "language";
    /// Follow the system language.
    pub const LANG_SYSTEM: &str = "lang_system";
    /// Switch language to Chinese.
    pub const LANG_ZH: &str = "lang_zh";
    /// Switch language to English.
    pub const LANG_EN: &str = "lang_en";
    /// Open about URL.
    pub const ABOUT: &str = "about";
    /// Exit process.
    pub const EXIT: &str = "exit";
    /// Grayed placeholder shown when no endpoint was enumerated at all.
    pub const NO_DEVICES: &str = "no_devices";

    /// Build the menu ID for a hotkey toggle, e.g. `hotkey_mute`.
    #[must_use]
    pub fn hotkey(action: HotkeyAction) -> String {
        format!("hotkey_{}", action.config_key())
    }

    /// Parse a `hotkey_*` ID back into its action, or `None`.
    #[must_use]
    pub fn parse_hotkey(id: &str) -> Option<HotkeyAction> {
        let key = id.strip_prefix("hotkey_")?;
        HotkeyAction::ALL
            .iter()
            .copied()
            .find(|action| action.config_key() == key)
    }

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

/// Grayed title `{DisplayName} v{ver}`; `CARGO_PKG_VERSION` is the single source.
#[must_use]
pub fn title_text() -> String {
    format!(
        "{} v{}",
        crate::TOOL_DISPLAY_NAME,
        env!("CARGO_PKG_VERSION")
    )
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
    /// Language mode (`System`/`Zh`/`En`) labels were built for.
    lang_mode: Lang,
    /// Effective language labels were rendered in.
    lang_ui: Lang,
    /// Global mute toggle.
    mute: CheckMenuItem,
    /// Volume-limit enabled toggle.
    vol_enabled: CheckMenuItem,
    /// Volume-limit presets `(percent, item)`.
    vol_items: Vec<(u32, CheckMenuItem)>,
    /// Hotkey bindings the labels were built from (change → rebuild).
    hotkeys: Hotkeys,
    /// Autostart toggle (grayed when the state is `Unknown`).
    autostart: CheckMenuItem,
    /// Language mode switches (three-way group, exactly one checked).
    lang_system: CheckMenuItem,
    /// Language switches.
    lang_zh: CheckMenuItem,
    /// Language switches.
    lang_en: CheckMenuItem,
}

/// Snapshot of everything the menu renders, built once per pump tick.
///
/// Groups the eight `build_menu`/`sync_state` inputs so the tray boundary
/// stays a two-argument call. All fields are `Copy` (shared refs and flags),
/// so tests can derive variants with struct-update syntax (`..base`).
pub struct MenuState<'a> {
    /// Persisted config (volume-limit checks, language mode).
    pub cfg: &'a AppConfig,
    /// Output devices.
    pub devices: &'a [AudioDevice],
    /// Active output device id (shown checked).
    pub default_id: Option<&'a str>,
    /// Input devices.
    pub inputs: &'a [AudioDevice],
    /// Active input device id (shown checked).
    pub default_input_id: Option<&'a str>,
    /// Global mute toggle state.
    pub muted: bool,
    /// Autostart toggle state (`Unknown` renders grayed, never off).
    pub autostart: &'a AutostartState,
    /// Effective language labels render in; checks follow `cfg.lang`.
    pub ui_lang: Lang,
}

impl MenuHandles {
    /// Sanitize a device id the same way [`build_menu`] does.
    fn sanitize_id(id: &str) -> String {
        id.replace(['\0', '\n', '\r'], "_")
    }

    /// Apply the autostart state to the toggle: `Unknown` is grayed with a
    /// status note in the label (muda has no tooltip API), never default-off.
    fn apply_autostart(&self, autostart: &AutostartState, ui_lang: Lang) {
        match autostart {
            AutostartState::Enabled => {
                self.autostart.set_text(tr("autostart", ui_lang));
                self.autostart.set_enabled(true);
                self.autostart.set_checked(true);
            }
            AutostartState::Disabled => {
                self.autostart.set_text(tr("autostart", ui_lang));
                self.autostart.set_enabled(true);
                self.autostart.set_checked(false);
            }
            AutostartState::Unknown(_) => {
                self.autostart.set_text(tr("autostart_unknown", ui_lang));
                self.autostart.set_enabled(false);
                self.autostart.set_checked(false);
            }
        }
    }

    /// Apply state changes in place when the device list is unchanged.
    ///
    /// Returns `false` when a full rebuild is required: device added, removed,
    /// reordered, renamed, UI language/mode changed, or labels otherwise
    /// stale. The caller must then fall back to a full rebuild.
    #[must_use]
    pub fn sync_state(&mut self, state: &MenuState<'_>) -> bool {
        let MenuState {
            cfg,
            devices,
            default_id,
            inputs,
            default_input_id,
            muted,
            autostart,
            ui_lang,
        } = *state;
        if self.lang_mode != cfg.lang || self.lang_ui != ui_lang {
            return false;
        }
        // Hotkey labels embed the combination; muda has no cheap partial
        // relabel, so a binding change rebuilds instead of flipping a check.
        if self.hotkeys != cfg.hotkeys {
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
        self.apply_autostart(autostart, ui_lang);
        self.lang_system.set_checked(cfg.lang == Lang::System);
        self.lang_zh.set_checked(cfg.lang == Lang::Zh);
        self.lang_en.set_checked(cfg.lang == Lang::En);
        true
    }
}

/// Menu label for a hotkey toggle.
///
/// Shows the bound combination, or the action's default one while it is off —
/// the item then doubles as "switch this on and you get `Ctrl+Alt+M`".
fn hotkey_label(action: HotkeyAction, cfg: &AppConfig, ui_lang: Lang) -> String {
    let combo = cfg.hotkeys.get(action).unwrap_or(action.default_combo());
    format!("{} ({combo})", tr(action.i18n_key(), ui_lang))
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
            // Sanitized once: the menu id and the change-detection key share it.
            let key = MenuHandles::sanitize_id(&dev.id);
            let item = CheckMenuItem::with_id(
                format!("{prefix}{key}"),
                truncate_label(&dev.name, MAX_LABEL_CHARS),
                true,
                checked,
                None,
            );
            (key, dev.name.clone(), item)
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

/// Build the tray menu for `state`.
///
/// `default_id` is the currently active device; it is shown checked.
/// `autostart` renders the toggle (`Unknown` grayed); `ui_lang` is the
/// effective language labels render in while checks follow `cfg.lang`.
#[must_use]
pub fn build_menu(state: &MenuState<'_>) -> MenuHandles {
    let MenuState {
        cfg,
        devices,
        default_id,
        inputs,
        default_input_id,
        muted,
        autostart,
        ui_lang,
    } = *state;
    let title = MenuItem::with_id(id::TITLE, title_text(), false, None);

    let device_items = check_entries(devices, DEVICE_PREFIX, default_id);
    let input_items = check_entries(inputs, INPUT_PREFIX, default_input_id);

    let refresh = MenuItem::with_id(id::REFRESH, tr("refresh", ui_lang), true, None);
    let mute = CheckMenuItem::with_id(id::MUTE, tr("mute", ui_lang), true, muted, None);

    // Disabled section headers; ids avoid the device prefixes so the handler
    // never parses them as device actions even if they were clickable.
    let output_header =
        MenuItem::with_id("outputs_header", tr("output_devices", ui_lang), false, None);
    let input_header =
        MenuItem::with_id("inputs_header", tr("input_devices", ui_lang), false, None);

    let vol_enabled = CheckMenuItem::with_id(
        id::VOL_ENABLED,
        tr("enabled", ui_lang),
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
        Submenu::with_id_and_items("volume_limit", tr("volume_limit", ui_lang), true, &vol_refs)
            .expect("volume_limit submenu");

    // One toggle per bindable action; the label carries the combination, the
    // check carries whether it is bound (`HotkeyAction::default_combo` is what
    // switching it on binds).
    let hotkey_items: Vec<CheckMenuItem> = HotkeyAction::ALL
        .iter()
        .map(|action| {
            CheckMenuItem::with_id(
                id::hotkey(*action),
                hotkey_label(*action, cfg, ui_lang),
                true,
                cfg.hotkeys.get(*action).is_some(),
                None,
            )
        })
        .collect();
    let hotkey_refs: Vec<&dyn muda::IsMenuItem> = hotkey_items
        .iter()
        .map(|item| item as &dyn muda::IsMenuItem)
        .collect();
    let hotkey_sub =
        Submenu::with_id_and_items(id::HOTKEYS, tr("hotkeys", ui_lang), true, &hotkey_refs)
            .expect("hotkeys submenu");

    let open_mixer = MenuItem::with_id(id::OPEN_MIXER, tr("open_mixer", ui_lang), true, None);
    let open_sound = MenuItem::with_id(id::OPEN_SOUND, tr("open_sound", ui_lang), true, None);
    let (autostart_label, autostart_enabled, autostart_checked) = match autostart {
        AutostartState::Enabled => (tr("autostart", ui_lang), true, true),
        AutostartState::Disabled => (tr("autostart", ui_lang), true, false),
        AutostartState::Unknown(_) => (tr("autostart_unknown", ui_lang), false, false),
    };
    let autostart_item = CheckMenuItem::with_id(
        id::AUTOSTART,
        autostart_label,
        autostart_enabled,
        autostart_checked,
        None,
    );
    let lang_system = CheckMenuItem::with_id(
        id::LANG_SYSTEM,
        tr("system", ui_lang),
        true,
        cfg.lang == Lang::System,
        None,
    );
    let lang_zh = CheckMenuItem::with_id(
        id::LANG_ZH,
        tr("chinese", ui_lang),
        true,
        cfg.lang == Lang::Zh,
        None,
    );
    let lang_en = CheckMenuItem::with_id(
        id::LANG_EN,
        tr("english", ui_lang),
        true,
        cfg.lang == Lang::En,
        None,
    );
    let lang_sub = Submenu::with_id_and_items(
        id::LANGUAGE,
        tr("language", ui_lang),
        true,
        &[&lang_system, &lang_zh, &lang_en],
    )
    .expect("language submenu");
    let about = MenuItem::with_id(id::ABOUT, tr("about", ui_lang), true, None);
    let exit = MenuItem::with_id(id::EXIT, tr("exit", ui_lang), true, None);

    let menu = Menu::new();
    let _ = menu.append(&title);
    let _ = menu.append(&PredefinedMenuItem::separator());
    if device_items.is_empty() && input_items.is_empty() {
        // Visible empty state: the device group must not vanish silently, or
        // a failed enumeration looks like a menu that lost its devices.
        let _ = menu.append(&MenuItem::with_id(
            id::NO_DEVICES,
            tr("no_devices", ui_lang),
            false,
            None,
        ));
    }
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
    let _ = menu.append(&hotkey_sub);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&open_mixer);
    let _ = menu.append(&open_sound);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&refresh);
    let _ = menu.append(&autostart_item);
    let _ = menu.append(&lang_sub);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&about);
    let _ = menu.append(&exit);

    MenuHandles {
        menu,
        device_items,
        input_items,
        lang_mode: cfg.lang,
        lang_ui: ui_lang,
        mute,
        vol_enabled,
        vol_items: VOLUME_PRESETS
            .iter()
            .zip(vol_items)
            .map(|(p, i)| (*p, i))
            .collect(),
        hotkeys: cfg.hotkeys.clone(),
        autostart: autostart_item,
        lang_system,
        lang_zh,
        lang_en,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_devices() -> Vec<AudioDevice> {
        vec![
            AudioDevice {
                id: "a".into(),
                name: "Speaker".into(),
            },
            AudioDevice {
                id: "b".into(),
                name: "Headset".into(),
            },
        ]
    }

    fn test_cfg() -> AppConfig {
        AppConfig::default()
    }

    /// Top-level action ids in menu order (separators carry none).
    fn menu_ids(menu: &Menu) -> Vec<String> {
        menu.items()
            .into_iter()
            .filter_map(|kind| match kind {
                muda::MenuItemKind::MenuItem(item) => Some(item.id().0.clone()),
                muda::MenuItemKind::Check(item) => Some(item.id().0.clone()),
                muda::MenuItemKind::Submenu(sub) => Some(sub.id().0.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn title_carries_display_name_and_version() {
        let title = title_text();
        assert!(title.starts_with(crate::TOOL_DISPLAY_NAME));
        assert!(title.contains(env!("CARGO_PKG_VERSION")));
        assert_eq!(
            title,
            format!(
                "{} v{}",
                crate::TOOL_DISPLAY_NAME,
                env!("CARGO_PKG_VERSION")
            )
        );
        // The menu head item renders exactly this string (same builder call).
        let head = MenuItem::with_id(id::TITLE, title_text(), false, None);
        assert_eq!(head.text(), title);
    }

    #[test]
    fn vol_preset_round_trip() {
        for preset in VOLUME_PRESETS {
            let menu_id = id::vol_preset(*preset);
            assert_eq!(id::parse_vol_preset(&menu_id), Some(*preset));
        }
        assert_eq!(id::parse_vol_preset("vol_30"), None);
        assert_eq!(id::parse_vol_preset("vol_x"), None);
        assert_eq!(id::parse_vol_preset("mute"), None);
    }

    #[test]
    fn three_way_lang_group_checks_mode() {
        for mode in [Lang::System, Lang::Zh, Lang::En] {
            let cfg = AppConfig {
                lang: mode,
                ..test_cfg()
            };
            let base = MenuState {
                cfg: &cfg,
                devices: &[],
                default_id: None,
                inputs: &[],
                default_input_id: None,
                muted: false,
                autostart: &AutostartState::Disabled,
                ui_lang: Lang::En,
            };
            let handles = build_menu(&base);
            assert_eq!(handles.lang_system.is_checked(), mode == Lang::System);
            assert_eq!(handles.lang_zh.is_checked(), mode == Lang::Zh);
            assert_eq!(handles.lang_en.is_checked(), mode == Lang::En);
        }
    }

    #[test]
    fn tail_group_order_is_fixed() {
        let cfg = test_cfg();
        let base = MenuState {
            cfg: &cfg,
            devices: &[],
            default_id: None,
            inputs: &[],
            default_input_id: None,
            muted: false,
            autostart: &AutostartState::Disabled,
            ui_lang: Lang::En,
        };
        let handles = build_menu(&base);
        // Separators carry no stable id; the last five actionable items are
        // the frozen tail: refresh → autostart → language → about → exit.
        let actionable = menu_ids(&handles.menu);
        assert!(actionable.len() >= 5, "{actionable:?}");
        assert_eq!(
            actionable[actionable.len() - 5..],
            [
                id::REFRESH,
                id::AUTOSTART,
                id::LANGUAGE,
                id::ABOUT,
                id::EXIT,
            ]
            .map(str::to_string),
        );
    }

    #[test]
    fn autostart_unknown_grayed_never_off() {
        let cfg = test_cfg();
        let base = MenuState {
            cfg: &cfg,
            devices: &[],
            default_id: None,
            inputs: &[],
            default_input_id: None,
            muted: false,
            autostart: &AutostartState::Disabled,
            ui_lang: Lang::En,
        };
        let unknown = build_menu(&MenuState {
            autostart: &AutostartState::Unknown("no read".into()),
            ..base
        });
        assert!(!unknown.autostart.is_enabled());
        assert!(!unknown.autostart.is_checked());
        assert!(unknown.autostart.text().contains("unknown"));

        let enabled = build_menu(&MenuState {
            autostart: &AutostartState::Enabled,
            ..base
        });
        assert!(enabled.autostart.is_enabled());
        assert!(enabled.autostart.is_checked());
    }

    #[test]
    fn sync_state_updates_checks_in_place() {
        let cfg = test_cfg();
        let ui = cfg.effective_lang();
        let devices = test_devices();
        let base = MenuState {
            cfg: &cfg,
            devices: &devices,
            default_id: Some("a"),
            inputs: &[],
            default_input_id: None,
            muted: false,
            autostart: &AutostartState::Disabled,
            ui_lang: ui,
        };
        let mut handles = build_menu(&base);
        assert!(handles.sync_state(&MenuState {
            default_id: Some("b"),
            muted: true,
            ..base
        }));
    }

    #[test]
    fn sync_state_rebuilds_on_rename_reorder_and_lang() {
        let cfg = test_cfg();
        let ui = cfg.effective_lang();
        let devices = test_devices();
        let base = MenuState {
            cfg: &cfg,
            devices: &devices,
            default_id: Some("a"),
            inputs: &[],
            default_input_id: None,
            muted: false,
            autostart: &AutostartState::Disabled,
            ui_lang: ui,
        };
        let mut handles = build_menu(&base);
        // Rename requires rebuild — labels are baked at build time.
        let mut renamed = devices.clone();
        renamed[0].name = "Renamed".into();
        assert!(!handles.sync_state(&MenuState {
            devices: &renamed,
            ..base
        }));
        // Reorder requires rebuild.
        let mut reordered = devices.clone();
        reordered.reverse();
        assert!(!handles.sync_state(&MenuState {
            devices: &reordered,
            ..base
        }));
        // Language change requires rebuild — all labels change.
        let other_ui = if ui == Lang::Zh { Lang::En } else { Lang::Zh };
        assert!(!handles.sync_state(&MenuState {
            ui_lang: other_ui,
            ..base
        }));
    }

    #[test]
    fn sync_state_tracks_input_devices() {
        let cfg = test_cfg();
        let ui = cfg.effective_lang();
        let devices = test_devices();
        let inputs = vec![
            AudioDevice {
                id: "m1".into(),
                name: "Mic".into(),
            },
            AudioDevice {
                id: "m2".into(),
                name: "Headset Mic".into(),
            },
        ];
        let base = MenuState {
            cfg: &cfg,
            devices: &devices,
            default_id: Some("a"),
            inputs: &inputs,
            default_input_id: Some("m1"),
            muted: false,
            autostart: &AutostartState::Disabled,
            ui_lang: ui,
        };
        let mut handles = build_menu(&base);
        // Default switch applies in place.
        assert!(handles.sync_state(&MenuState {
            default_input_id: Some("m2"),
            ..base
        }));
        // Input added requires rebuild.
        let mut grown = inputs.clone();
        grown.push(AudioDevice {
            id: "m3".into(),
            name: "Cam Mic".into(),
        });
        assert!(!handles.sync_state(&MenuState {
            inputs: &grown,
            default_input_id: Some("m2"),
            ..base
        }));
    }

    #[test]
    fn hotkey_id_round_trip() {
        for action in HotkeyAction::ALL {
            let menu_id = id::hotkey(action);
            assert_eq!(id::parse_hotkey(&menu_id), Some(action));
            // Distinct from the submenu id: the submenu never parses as an action.
            assert_ne!(menu_id, id::HOTKEYS);
        }
        assert_eq!(id::parse_hotkey(id::HOTKEYS), None);
        assert_eq!(id::parse_hotkey("hotkey_"), None);
        assert_eq!(id::parse_hotkey("hotkey_nope"), None);
    }

    #[test]
    fn hotkey_items_show_combo_and_binding_state() {
        let cfg = test_cfg();
        let base = MenuState {
            cfg: &cfg,
            devices: &[],
            default_id: None,
            inputs: &[],
            default_input_id: None,
            muted: false,
            autostart: &AutostartState::Disabled,
            ui_lang: Lang::En,
        };
        // The submenu carries one toggle per action, in `HotkeyAction::ALL` order.
        let children = |handles: &MenuHandles| -> Vec<(String, bool)> {
            let submenu = handles
                .menu
                .items()
                .into_iter()
                .find_map(|kind| match kind {
                    muda::MenuItemKind::Submenu(sub) if sub.id().0 == id::HOTKEYS => Some(sub),
                    _ => None,
                })
                .expect("hotkeys submenu");
            let out: Vec<(String, bool)> = submenu
                .items()
                .into_iter()
                .filter_map(|kind| match kind {
                    muda::MenuItemKind::Check(item) => {
                        Some((item.text().clone(), item.is_checked()))
                    }
                    _ => None,
                })
                .collect();
            out
        };

        let opted_out = children(&build_menu(&base));
        assert_eq!(opted_out.len(), HotkeyAction::ALL.len());
        for (action, (label, checked)) in HotkeyAction::ALL.iter().zip(&opted_out) {
            assert!(!checked, "hotkeys are opt-in");
            // An off action still advertises the combo that switching it on binds.
            assert!(label.contains(action.default_combo()), "{label}");
        }

        let bound = AppConfig {
            hotkeys: Hotkeys {
                mute: Some("Ctrl+Shift+F9".into()),
                ..Hotkeys::default()
            },
            ..test_cfg()
        };
        let (label, checked) = children(&build_menu(&MenuState {
            cfg: &bound,
            ..base
        }))
        .into_iter()
        .next()
        .expect("mute item");
        assert!(checked);
        assert!(label.contains("Ctrl+Shift+F9"), "{label}");
    }

    #[test]
    fn hotkey_binding_change_forces_rebuild() {
        let cfg = test_cfg();
        let ui = cfg.effective_lang();
        let base = MenuState {
            cfg: &cfg,
            devices: &[],
            default_id: None,
            inputs: &[],
            default_input_id: None,
            muted: false,
            autostart: &AutostartState::Disabled,
            ui_lang: ui,
        };
        let mut handles = build_menu(&base);
        assert!(handles.sync_state(&base));
        // Labels embed the combination: binding one must relabel → rebuild.
        let bound = AppConfig {
            hotkeys: Hotkeys {
                volume_up: Some("Ctrl+Alt+Up".into()),
                ..Hotkeys::default()
            },
            ..test_cfg()
        };
        assert!(!handles.sync_state(&MenuState {
            cfg: &bound,
            ..base
        }));
    }

    #[test]
    fn empty_enumeration_shows_placeholder() {
        let cfg = test_cfg();
        let base = MenuState {
            cfg: &cfg,
            devices: &[],
            default_id: None,
            inputs: &[],
            default_input_id: None,
            muted: false,
            autostart: &AutostartState::Disabled,
            ui_lang: Lang::En,
        };
        let empty = build_menu(&base);
        assert!(menu_ids(&empty.menu).contains(&id::NO_DEVICES.to_string()));

        let devices = test_devices();
        let populated = build_menu(&MenuState {
            devices: &devices,
            default_id: Some("a"),
            ..base
        });
        assert!(!menu_ids(&populated.menu).contains(&id::NO_DEVICES.to_string()));
    }
}

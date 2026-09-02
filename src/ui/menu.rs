//! Tray context menu builder.

use muda::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};

use crate::audio::AudioDevice;
use crate::config::{AppConfig, Lang};
use crate::ui::i18n::tr;
use crate::ui::text::{MAX_LABEL_CHARS, truncate_label};

/// Prefix for per-device menu item IDs; the remainder is the WASAPI endpoint ID.
pub const DEVICE_PREFIX: &str = "device_";
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
pub struct MenuHandles {
    /// The root menu attached to the tray.
    pub menu: Menu,
}

/// Build the tray menu for `cfg` / `devices`.
///
/// `default_id` is the currently active device; it is shown checked.
#[must_use]
pub fn build_menu(
    cfg: &AppConfig,
    devices: &[AudioDevice],
    default_id: Option<&str>,
    muted: bool,
) -> MenuHandles {
    let lang = cfg.lang;

    // Sanitize device id for muda (no NUL/control).
    let sanitize_id = |id: &str| -> String { id.replace(['\0', '\n', '\r'], "_") };
    let device_items: Vec<CheckMenuItem> = devices
        .iter()
        .map(|dev| {
            let checked = default_id == Some(dev.id.as_str());
            CheckMenuItem::with_id(
                format!("{DEVICE_PREFIX}{}", sanitize_id(&dev.id)),
                truncate_label(&dev.name, MAX_LABEL_CHARS),
                true,
                checked,
                None,
            )
        })
        .collect();

    let mute = CheckMenuItem::with_id(id::MUTE, tr("mute", lang), true, muted, None);

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
    for item in &device_items {
        let _ = menu.append(item);
    }
    if !device_items.is_empty() {
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

    MenuHandles { menu }
}

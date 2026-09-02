//! Tray context menu builder.

use muda::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};

use crate::audio::AudioDevice;
use crate::config::{AppConfig, Lang};
use crate::ui::i18n::tr;

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

    // Sanitize device id for muda (no NUL/control) and truncate long names.
    let sanitize_id = |id: &str| -> String { id.replace(['\0', '\n', '\r'], "_") };
    let truncate_name = |name: &str| -> String {
        let sanitized = name.replace(['\r', '\n', '\t'], " ");
        if sanitized.chars().count() > 60 {
            let t: String = sanitized.chars().take(58).collect();
            format!("{t}…")
        } else {
            sanitized
        }
    };
    let device_items: Vec<CheckMenuItem> = devices
        .iter()
        .map(|dev| {
            let checked = default_id == Some(dev.id.as_str());
            CheckMenuItem::with_id(
                format!("device_{}", sanitize_id(&dev.id)),
                truncate_name(&dev.name),
                true,
                checked,
                None,
            )
        })
        .collect();

    let mute = CheckMenuItem::with_id("mute", tr("mute", lang), true, muted, None);

    let vol_enabled = CheckMenuItem::with_id(
        "vol_enabled",
        tr("enabled", lang),
        true,
        cfg.volume_limit_enabled,
        None,
    );
    let vol_25 = CheckMenuItem::with_id(
        "vol_25",
        "25%",
        cfg.volume_limit_enabled,
        cfg.volume_limit == 25 && cfg.volume_limit_enabled,
        None,
    );
    let vol_50 = CheckMenuItem::with_id(
        "vol_50",
        "50%",
        cfg.volume_limit_enabled,
        cfg.volume_limit == 50 && cfg.volume_limit_enabled,
        None,
    );
    let vol_75 = CheckMenuItem::with_id(
        "vol_75",
        "75%",
        cfg.volume_limit_enabled,
        cfg.volume_limit == 75 && cfg.volume_limit_enabled,
        None,
    );
    let vol_sub = Submenu::with_id_and_items(
        "volume_limit",
        tr("volume_limit", lang),
        true,
        &[&vol_enabled, &PredefinedMenuItem::separator(), &vol_25, &vol_50, &vol_75],
    )
    .expect("volume_limit submenu");

    let wheel = CheckMenuItem::with_id(
        "wheel_accel",
        tr("wheel_accel", lang),
        true,
        cfg.wheel_acceleration,
        None,
    );
    let exp_sub =
        Submenu::with_id_and_items("experimental", tr("experimental", lang), true, &[&wheel])
            .expect("experimental submenu");

    let open_mixer = MenuItem::with_id("open_mixer", tr("open_mixer", lang), true, None);
    let open_sound = MenuItem::with_id("open_sound", tr("open_sound", lang), true, None);
    let autostart =
        CheckMenuItem::with_id("autostart", tr("autostart", lang), true, cfg.autostart, None);
    let lang_zh =
        CheckMenuItem::with_id("lang_zh", tr("chinese", lang), true, cfg.lang == Lang::Zh, None);
    let lang_en =
        CheckMenuItem::with_id("lang_en", tr("english", lang), true, cfg.lang == Lang::En, None);
    let lang_sub = Submenu::with_id_and_items("language", "Language", true, &[&lang_zh, &lang_en])
        .expect("language submenu");
    let about = MenuItem::with_id("about", tr("about", lang), true, None);
    let exit = MenuItem::with_id("exit", tr("exit", lang), true, None);

    let menu = Menu::new();
    for item in &device_items {
        let _ = menu.append(item);
    }
    if !device_items.is_empty() {
        let _ = menu.append(&PredefinedMenuItem::separator());
    }
    let _ = menu.append(&mute);
    let _ = menu.append(&vol_sub);
    let _ = menu.append(&exp_sub);
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

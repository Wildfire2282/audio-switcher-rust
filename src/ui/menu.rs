use muda::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};

use crate::audio::AudioDevice;
use crate::config::AppConfig;
use crate::ui::i18n::tr;

pub struct MenuHandles {
    pub menu: Menu,
}

pub fn build_menu(
    cfg: &AppConfig,
    devices: &[AudioDevice],
    default_id: Option<&str>,
    muted: bool,
) -> MenuHandles {
    let lang = cfg.lang.as_str();

    let mut device_items: Vec<CheckMenuItem> = Vec::new();
    for dev in devices {
        let checked = default_id == Some(dev.id.as_str());
        let item = CheckMenuItem::with_id(
            format!("device_{}", dev.id),
            dev.name.clone(),
            true,
            checked,
            None,
        );
        device_items.push(item);
    }

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
    let vol_custom = MenuItem::with_id("vol_custom", tr("custom", lang), cfg.volume_limit_enabled, None);
    let vol_sub = Submenu::with_id_and_items(
        "volume_limit",
        tr("volume_limit", lang),
        true,
        &[&vol_enabled, &PredefinedMenuItem::separator(), &vol_25, &vol_50, &vol_custom],
    )
    .unwrap();

    let wheel = CheckMenuItem::with_id(
        "wheel_accel",
        tr("wheel_accel", lang),
        true,
        cfg.wheel_acceleration,
        None,
    );
    let exp_sub =
        Submenu::with_id_and_items("experimental", tr("experimental", lang), true, &[&wheel])
            .unwrap();

    let open_mixer = MenuItem::with_id("open_mixer", tr("open_mixer", lang), true, None);
    let open_sound = MenuItem::with_id("open_sound", tr("open_sound", lang), true, None);
    let autostart =
        CheckMenuItem::with_id("autostart", tr("autostart", lang), true, cfg.autostart, None);
    let lang_zh =
        CheckMenuItem::with_id("lang_zh", tr("chinese", lang), true, cfg.lang == "zh", None);
    let lang_en =
        CheckMenuItem::with_id("lang_en", tr("english", lang), true, cfg.lang == "en", None);
    let lang_sub =
        Submenu::with_id_and_items("language", "Language", true, &[&lang_zh, &lang_en]).unwrap();
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

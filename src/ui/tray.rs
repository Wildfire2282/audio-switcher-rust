use tray_icon::{TrayIcon, TrayIconBuilder};

use crate::audio::AudioDevice;
use crate::config::AppConfig;
use crate::ui::icon::make_icon;
use crate::ui::menu::{build_menu, MenuHandles};
use crate::ui::tooltip::format_tooltip;

pub struct TrayWrapper {
    pub tray: TrayIcon,
    pub handles: MenuHandles,
}

impl TrayWrapper {
    pub fn new(cfg: &AppConfig, devices: &[AudioDevice], default_id: Option<&str>, muted: bool) -> Self {
        let handles = build_menu(cfg, devices, default_id, muted);
        let icon = make_icon(muted);
        let tooltip = format_tooltip(
            default_id.and_then(|id| devices.iter().find(|d| d.id == id)),
            50,
            muted,
            &cfg.lang,
        );
        let tray = TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip(tooltip)
            .with_menu(Box::new(handles.menu.clone()))
            .with_menu_on_left_click(false)
            .build()
            .expect("tray build failed");
        Self { tray, handles }
    }

    pub fn update_tooltip(&self, text: String) {
        let _ = self.tray.set_tooltip(Some(text));
    }

    pub fn update_icon(&self, muted: bool) {
        let icon = make_icon(muted);
        let _ = self.tray.set_icon(Some(icon));
    }

    pub fn rebuild_menu(
        &mut self,
        cfg: &AppConfig,
        devices: &[AudioDevice],
        default_id: Option<&str>,
        muted: bool,
    ) {
        let new_handles = build_menu(cfg, devices, default_id, muted);
        self.tray.set_menu(Some(Box::new(new_handles.menu.clone())));
        self.handles = new_handles;
    }
}

pub fn open_volume_mixer() {
    if crate::platform::shell::open_file("SndVol.exe", None).is_err() {
        crate::platform::shell::show_error("打开音量合成器失败");
    }
}

pub fn open_sound_settings() {
    if crate::platform::shell::open_file("control", Some("mmsys.cpl")).is_err() {
        crate::platform::shell::show_error("打开声音设置失败");
    }
}

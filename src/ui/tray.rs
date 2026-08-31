use tray_icon::{TrayIcon, TrayIconBuilder};

use crate::audio::AudioDevice;
use crate::config::AppConfig;
use crate::ui::icon::make_icon;
use crate::ui::menu::{build_menu, MenuHandles};
use crate::ui::tooltip::format_tooltip;

/// Wrapper around `tray-icon`'s `TrayIcon` holding the menu handles.
pub struct TrayWrapper {
    /// Underlying tray icon.
    pub tray: TrayIcon,
    /// Handles to keep the menu alive.
    pub handles: MenuHandles,
}

impl TrayWrapper {
    /// Build a new tray icon and menu for the current config/devices.
    ///
    /// # Errors
    ///
    /// Returns `Err` when the underlying tray icon cannot be created.
    pub fn new(
        cfg: &AppConfig,
        devices: &[AudioDevice],
        default_id: Option<&str>,
        muted: bool,
    ) -> Result<Self, String> {
        let handles = build_menu(cfg, devices, default_id, muted);
        let icon = make_icon(muted);
        let tooltip = format_tooltip(
            default_id.and_then(|id| devices.iter().find(|d| d.id == id)),
            50,
            muted,
            cfg.lang,
        );
        let tray = TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip(tooltip)
            .with_menu(Box::new(handles.menu.clone()))
            .with_menu_on_left_click(false)
            .build()
            .map_err(|e| format!("tray build failed: {e}"))?;
        Ok(Self { tray, handles })
    }

    /// Update the tooltip text (logs on failure).
    pub fn update_tooltip(&self, text: String) {
        if let Err(e) = self.tray.set_tooltip(Some(text)) {
            eprintln!("tray set_tooltip failed: {e:?}");
        }
    }

    /// Update the tray icon for mute state (logs on failure).
    pub fn update_icon(&self, muted: bool) {
        let icon = make_icon(muted);
        if let Err(e) = self.tray.set_icon(Some(icon)) {
            eprintln!("tray set_icon failed: {e:?}");
        }
    }

    /// Rebuild the context menu from current config/devices (logs on failure).
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

/// Open the system volume mixer.
pub fn open_volume_mixer() {
    if crate::platform::shell::open_file("SndVol.exe", None).is_err() {
        crate::platform::shell::show_error("打开音量合成器失败");
    }
}

/// Open the system sound settings.
pub fn open_sound_settings() {
    if crate::platform::shell::open_file("control", Some("mmsys.cpl")).is_err() {
        crate::platform::shell::show_error("打开声音设置失败");
    }
}

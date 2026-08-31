use tray_icon::{TrayIcon, TrayIconBuilder};

use crate::audio::AudioDevice;
use crate::config::AppConfig;
use crate::ui::icon::make_icon;
use crate::ui::menu::{build_menu, MenuHandles};
use crate::ui::theme::is_dark_mode;
use crate::ui::tooltip::format_tooltip;

pub struct TrayWrapper {
    pub tray: TrayIcon,
    pub handles: MenuHandles,
}

impl TrayWrapper {
    pub fn new(cfg: &AppConfig, devices: &[AudioDevice], default_id: Option<&str>, muted: bool) -> Self {
        let handles = build_menu(cfg, devices, default_id, muted);
        let icon = make_icon(muted, is_dark_mode());
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
        let icon = make_icon(muted, is_dark_mode());
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

#[cfg(windows)]
pub fn open_volume_mixer() {
    unsafe {
        use windows::core::PCWSTR;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONWARNING, MB_OK};
        let op: Vec<u16> = "open\0".encode_utf16().collect();
        let file: Vec<u16> = "SndVol.exe\0".encode_utf16().collect();
        let res = ShellExecuteW(
            None,
            PCWSTR(op.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        );
        if res.0 as isize <= 32 {
            let msg: Vec<u16> = "打开音量合成器失败\0".encode_utf16().collect();
            let title: Vec<u16> = "Audio Switcher\0".encode_utf16().collect();
            MessageBoxW(None, PCWSTR(msg.as_ptr()), PCWSTR(title.as_ptr()), MB_OK | MB_ICONWARNING);
        }
    }
}

#[cfg(not(windows))]
pub fn open_volume_mixer() {}

#[cfg(windows)]
pub fn open_sound_settings() {
    unsafe {
        use windows::core::PCWSTR;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONWARNING, MB_OK};
        let op: Vec<u16> = "open\0".encode_utf16().collect();
        let file: Vec<u16> = "control\0".encode_utf16().collect();
        let params: Vec<u16> = "mmsys.cpl\0".encode_utf16().collect();
        let res = ShellExecuteW(
            None,
            PCWSTR(op.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR(params.as_ptr()),
            PCWSTR::null(),
            windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        );
        if res.0 as isize <= 32 {
            let msg: Vec<u16> = "打开声音设置失败\0".encode_utf16().collect();
            let title: Vec<u16> = "Audio Switcher\0".encode_utf16().collect();
            MessageBoxW(None, PCWSTR(msg.as_ptr()), PCWSTR(title.as_ptr()), MB_OK | MB_ICONWARNING);
        }
    }
}

#[cfg(not(windows))]
pub fn open_sound_settings() {}

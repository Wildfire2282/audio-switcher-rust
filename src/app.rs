#![allow(clippy::too_many_lines)]

use std::time::{Duration, Instant};

use crate::audio::{AudioBackend, RealBackend};
use crate::config::AppConfig;
use crate::platform::hook;
use crate::ui::{format_tooltip, TrayWrapper, WheelState};

#[cfg(windows)]
use windows::core::PCWSTR;

/// App owns all runtime state. Replaces the scattered `static Atomic*` + procedural `main` loop.
pub struct App {
    cfg: AppConfig,
    backend: RealBackend,
    tray: TrayWrapper,
    wheel: WheelState,
    is_hover: bool,
    last_hover: Instant,
    last_cursor_check: Instant,
    last_cursor_over: bool,
    last_dark: bool,
    last_theme_check: Instant,
    hook: Option<hook::WheelHook>,
    hook_install_at: Instant,
    _com: crate::platform::ComGuard,
}

impl App {
    pub fn new(com: crate::platform::ComGuard) -> Self {
        let cfg = AppConfig::load();
        // background autostart check (non-blocking)
        let autostart = cfg.autostart;
        std::thread::spawn(move || {
            if autostart && !crate::system::is_autostart_enabled() {
                let _ = crate::system::set_autostart(true);
            }
        });

        // fast first frame: placeholder tray before COM enumeration
        let mut tray = TrayWrapper::new(&cfg, &[], None, false);
        let mut backend = RealBackend::new();
        let snap = backend.fetch_snapshot_clamped(&cfg);
        let default_id = snap.default_device.as_ref().map(|d| d.id.clone());
        tray.rebuild_menu(&cfg, &snap.devices, default_id.as_deref(), snap.mute);
        tray.update_tooltip(format_tooltip(
            snap.default_device.as_ref(),
            snap.volume,
            snap.mute,
            &cfg.lang,
        ));
        tray.update_icon(snap.mute);

        let last_dark = crate::ui::is_dark_mode();

        Self {
            cfg,
            backend,
            tray,
            wheel: WheelState::new(),
            is_hover: false,
            last_hover: Instant::now() - Duration::from_secs(10),
            last_cursor_check: Instant::now() - Duration::from_secs(1),
            last_cursor_over: false,
            last_dark,
            last_theme_check: Instant::now(),
            hook: None,
            hook_install_at: Instant::now() + Duration::from_millis(180),
            _com: com,
        }
    }

    /// Deduplicated UI refresh after any state change.
    fn refresh_ui(&mut self) {
        let devs = self.backend.enumerate_devices().unwrap_or_default();
        let def = self.backend.get_default_device();
        let mute = self.backend.get_mute().unwrap_or(false);
        self.tray.rebuild_menu(
            &self.cfg,
            &devs,
            def.as_ref().map(|d| d.id.as_str()),
            mute,
        );
        self.update_tooltip_and_icon();
    }

    fn update_tooltip_and_icon(&self) {
        // batch volume+mute in one Activate
        #[cfg(windows)]
        if let Ok((vol, mute)) = self.backend.get_volume_and_mute() {
            let dev = self.backend.get_default_device();
            let tip = format_tooltip(dev.as_ref(), vol, mute, &self.cfg.lang);
            self.tray.update_tooltip(tip);
            self.tray.update_icon(mute);
            return;
        }
        let dev = self.backend.get_default_device();
        let vol = self.backend.get_volume().unwrap_or(0);
        let mute = self.backend.get_mute().unwrap_or(false);
        let tip = format_tooltip(dev.as_ref(), vol, mute, &self.cfg.lang);
        self.tray.update_tooltip(tip);
        self.tray.update_icon(mute);
    }

    fn handle_menu(&mut self, id: &str) {
        if let Some(dev_id) = id.strip_prefix("device_") {
            match self.backend.set_default_device(dev_id) {
                Ok(()) => {
                    let _ = self.backend.clamp_volume_if_needed(&self.cfg);
                    self.refresh_ui();
                }
                Err(e) => {
                    #[cfg(windows)]
                    unsafe {
                        let msg = format!("切换设备失败: {}", e);
                        let wide: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
                        let title: Vec<u16> = "Audio Switcher\0".encode_utf16().collect();
                        windows::Win32::UI::WindowsAndMessaging::MessageBoxW(
                            None,
                            PCWSTR(wide.as_ptr()),
                            PCWSTR(title.as_ptr()),
                            windows::Win32::UI::WindowsAndMessaging::MB_OK
                                | windows::Win32::UI::WindowsAndMessaging::MB_ICONWARNING,
                        );
                    }
                    #[cfg(not(windows))]
                    let _ = e;
                }
            }
            return;
        }
        match id {
            "mute" => {
                if let Ok(m) = self.backend.get_mute() {
                    let _ = self.backend.set_mute(!m);
                    self.refresh_ui();
                }
            }
            "vol_enabled" => {
                self.cfg.volume_limit_enabled = !self.cfg.volume_limit_enabled;
                let _ = self.cfg.save();
                if self.cfg.volume_limit_enabled {
                    let _ = self.backend.clamp_volume_if_needed(&self.cfg);
                }
                self.refresh_ui();
            }
            "vol_25" | "vol_50" => {
                self.cfg.volume_limit = if id == "vol_25" { 25 } else { 50 };
                self.cfg.volume_limit_enabled = true;
                let _ = self.cfg.save();
                let _ = self.backend.clamp_volume_if_needed(&self.cfg);
                self.refresh_ui();
            }
            "vol_custom" => {
                if let Some(v) = crate::platform::prompt_custom_limit(&self.cfg.lang) {
                    self.cfg.volume_limit = v;
                    self.cfg.volume_limit_enabled = true;
                    let _ = self.cfg.save();
                    let _ = self.backend.clamp_volume_if_needed(&self.cfg);
                    self.refresh_ui();
                }
            }
            "wheel_accel" => {
                self.cfg.wheel_acceleration = !self.cfg.wheel_acceleration;
                let _ = self.cfg.save();
                // only menu rebuild, no tooltip
                let devs = self.backend.enumerate_devices().unwrap_or_default();
                let def = self.backend.get_default_device();
                self.tray.rebuild_menu(
                    &self.cfg,
                    &devs,
                    def.as_ref().map(|d| d.id.as_str()),
                    self.backend.get_mute().unwrap_or(false),
                );
            }
            "open_mixer" => crate::ui::open_volume_mixer(),
            "open_sound" => crate::ui::open_sound_settings(),
            "autostart" => {
                let new_val = !self.cfg.autostart;
                match crate::system::set_autostart(new_val) {
                    Ok(()) => {
                        self.cfg.autostart = new_val;
                        let _ = self.cfg.save();
                        self.refresh_ui();
                    }
                    Err(_) => crate::platform::show_autostart_error(),
                }
            }
            "lang_zh" | "lang_en" => {
                self.cfg.lang = if id == "lang_zh" { "zh" } else { "en" }.to_string();
                let _ = self.cfg.save();
                self.refresh_ui();
            }
            "about" => {
                #[cfg(windows)]
                unsafe {
                    let url: Vec<u16> =
                        "https://github.com/Wildfire2282/audio-switcher-rust\0".encode_utf16().collect();
                    let op: Vec<u16> = "open\0".encode_utf16().collect();
                    let _ = windows::Win32::UI::Shell::ShellExecuteW(
                        None,
                        PCWSTR(op.as_ptr()),
                        PCWSTR(url.as_ptr()),
                        PCWSTR::null(),
                        PCWSTR::null(),
                        windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
                    );
                }
            }
            "exit" => {
                // `Drop` of WheelHook + ComGuard handles cleanup
                std::process::exit(0);
            }
            _ => {}
        }
    }

    pub fn run(mut self) {
        let menu_rx = muda::MenuEvent::receiver();
        let tray_rx = tray_icon::TrayIconEvent::receiver();

        loop {
            #[cfg(windows)]
            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::{
                    DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
                };
                let mut msg = MSG::default();
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }

            // delayed hook install on thread with message pump
            if self.hook.is_none() && Instant::now() >= self.hook_install_at {
                self.hook = hook::WheelHook::install();
            }

            // theme polling fallback (2s TTL cache already)
            if self.last_theme_check.elapsed() > Duration::from_millis(1800) {
                self.last_theme_check = Instant::now();
                let dark = crate::ui::is_dark_mode();
                if dark != self.last_dark {
                    self.last_dark = dark;
                    #[cfg(windows)]
                    crate::ui::invalidate_dark_mode_cache();
                    if let Ok(m) = self.backend.get_mute() {
                        self.tray.update_icon(m);
                    }
                }
            }

            // tray events
            if let Ok(event) = tray_rx.try_recv() {
                match event {
                    tray_icon::TrayIconEvent::Click {
                        button: tray_icon::MouseButton::Middle,
                        button_state: tray_icon::MouseButtonState::Up,
                        ..
                    } => {
                        if let Ok(m) = self.backend.get_mute() {
                            let _ = self.backend.set_mute(!m);
                            self.refresh_ui();
                        }
                        self.is_hover = true;
                        self.last_hover = Instant::now();
                        self.wheel.clear();
                    }
                    tray_icon::TrayIconEvent::Enter { .. }
                    | tray_icon::TrayIconEvent::Move { .. }
                    | tray_icon::TrayIconEvent::Click { .. } => {
                        self.is_hover = true;
                        self.last_hover = Instant::now();
                        self.wheel.clear();
                    }
                    tray_icon::TrayIconEvent::Leave { .. } => {
                        self.is_hover = false;
                        self.wheel.clear();
                    }
                    tray_icon::TrayIconEvent::DoubleClick { .. } => {
                        self.is_hover = true;
                        self.last_hover = Instant::now();
                        self.wheel.clear();
                    }
                    _ => {}
                }
            }

            #[cfg(windows)]
            {
                let need_cursor = hook::peek_pending()
                    || self.is_hover
                    || self.last_hover.elapsed() < Duration::from_millis(2500);
                if need_cursor && self.last_cursor_check.elapsed() > Duration::from_millis(80) {
                    self.last_cursor_check = Instant::now();
                    let over = hook::cursor_over_tray(&self.tray);
                    self.last_cursor_over = over;
                    if over {
                        if !self.is_hover {
                            self.is_hover = true;
                            self.wheel.clear();
                        }
                        self.last_hover = Instant::now();
                    }
                }
            }

            if let Ok(event) = menu_rx.try_recv() {
                let id = event.id.0.clone();
                self.handle_menu(&id);
            }

            if hook::take_pending() {
                let delta = hook::take_delta();
                if (self.is_hover
                    || self.last_hover.elapsed() < Duration::from_millis(2500)
                    || self.last_cursor_over)
                    && delta != 0
                {
                    self.last_hover = Instant::now();
                    let now = Instant::now();
                    let step = self.wheel.push(now, self.cfg.wheel_acceleration, delta);
                    let total = WheelState::total_step(delta, step);
                    if let Ok(vol) = self.backend.get_volume() {
                        let new_vol = (vol as i32 + total).clamp(0, 100) as u32;
                        let clamped = crate::config::clamp_volume(new_vol, &self.cfg);
                        let _ = self.backend.set_volume(clamped);
                        self.update_tooltip_and_icon();
                    }
                }
            }

            if self.backend.poll_device_changed() || crate::audio::take_device_changed() {
                let _ = self.backend.clamp_volume_if_needed(&self.cfg);
                self.refresh_ui();
            }

            #[cfg(windows)]
            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::{
                    MsgWaitForMultipleObjectsEx, MWMO_INPUTAVAILABLE, QS_ALLINPUT,
                };
                let has_pending = hook::peek_pending();
                if has_pending {
                    let _ = MsgWaitForMultipleObjectsEx(Some(&[]), 8, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
                } else {
                    let remaining = self
                        .last_theme_check
                        .checked_add(Duration::from_millis(1800))
                        .and_then(|t| t.checked_duration_since(Instant::now()))
                        .map(|d| d.as_millis() as u32)
                        .unwrap_or(0)
                        .min(500);
                    let timeout = remaining.max(8);
                    let _ = MsgWaitForMultipleObjectsEx(Some(&[]), timeout, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
                }
            }
            #[cfg(not(windows))]
            {
                let has_pending = hook::peek_pending();
                if has_pending {
                    std::thread::sleep(Duration::from_millis(8));
                } else {
                    std::thread::sleep(Duration::from_millis(24));
                }
            }
        }
    }
}

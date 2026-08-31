pub mod handler;

use std::time::{Duration, Instant};

use crate::audio::{AudioBackend, RealBackend};
use crate::config::AppConfig;
use crate::platform::hook;
use crate::ui::{format_tooltip, TrayWrapper, WheelState};
use handler::MenuAction;

/// App owns all runtime state. Generic over `AudioBackend` for test injection.
pub struct App<B: AudioBackend = RealBackend> {
    cfg: AppConfig,
    backend: B,
    tray: TrayWrapper,
    wheel: WheelState,
    is_hover: bool,
    last_hover: Instant,
    last_cursor_check: Instant,
    last_cursor_over: bool,
    last_devices_rebuild: Instant,
    hook: Option<hook::WheelHook>,
    hook_install_at: Instant,
    _com: crate::platform::ComGuard,
}

impl App<RealBackend> {
    pub fn new(com: crate::platform::ComGuard) -> Self {
        Self::with_backend(com, RealBackend::new())
    }
}

impl<B: AudioBackend> App<B> {
    pub fn with_backend(com: crate::platform::ComGuard, mut backend: B) -> Self {
        let cfg = AppConfig::load();
        let autostart = cfg.autostart;
        std::thread::spawn(move || {
            if autostart && !crate::platform::is_autostart_enabled() {
                let _ = crate::platform::set_autostart(true);
            }
        });
        let mut tray = TrayWrapper::new(&cfg, &[], None, false);
        let snap = backend.fetch_snapshot_clamped(&cfg);
        let default_id = snap.default_device.as_ref().map(|d| d.id.clone());
        tray.rebuild_menu(&cfg, &snap.devices, default_id.as_deref(), snap.mute);
        tray.update_tooltip(format_tooltip(snap.default_device.as_ref(), snap.volume, snap.mute, &cfg.lang));
        tray.update_icon(snap.mute);
        Self {
            cfg,
            backend,
            tray,
            wheel: WheelState::new(),
            is_hover: false,
            last_hover: Instant::now() - Duration::from_secs(10),
            last_cursor_check: Instant::now() - Duration::from_secs(1),
            last_cursor_over: false,
            last_devices_rebuild: Instant::now() - Duration::from_secs(1),
            hook: None,
            hook_install_at: Instant::now() + Duration::from_millis(180),
            _com: com,
        }
    }

    fn refresh_ui(&mut self) {
        let devs = self.backend.enumerate_devices().unwrap_or_default();
        let def = self.backend.get_default_device();
        let mute = self.backend.get_mute().unwrap_or(false);
        self.tray.rebuild_menu(&self.cfg, &devs, def.as_ref().map(|d| d.id.as_str()), mute);
        self.update_tooltip_and_icon();
    }

    fn update_tooltip_and_icon(&self) {
        #[cfg(windows)]
        if let Ok((vol, mute)) = self.backend.get_volume_and_mute() {
            let dev = self.backend.get_default_device();
            self.tray.update_tooltip(format_tooltip(dev.as_ref(), vol, mute, &self.cfg.lang));
            self.tray.update_icon(mute);
            return;
        }
        let dev = self.backend.get_default_device();
        let vol = self.backend.get_volume().unwrap_or(0);
        let mute = self.backend.get_mute().unwrap_or(false);
        self.tray.update_tooltip(format_tooltip(dev.as_ref(), vol, mute, &self.cfg.lang));
        self.tray.update_icon(mute);
    }

    fn save_and_refresh(&mut self, clamp: bool) {
        let _ = self.cfg.save();
        if clamp && self.cfg.volume_limit_enabled {
            let _ = self.backend.clamp_volume_if_needed(&self.cfg);
        }
        self.refresh_ui();
    }

    fn handle_menu(&mut self, id: &str) {
        match MenuAction::from_id(id) {
            MenuAction::Device(dev_id) => match self.backend.set_default_device(&dev_id) {
                Ok(()) => {
                    let _ = self.backend.clamp_volume_if_needed(&self.cfg);
                    self.refresh_ui();
                }
                Err(e) => {
                    crate::platform::shell::show_error(&format!("切换设备失败: {}", e));
                }
            },
            MenuAction::Mute => {
                if let Ok(m) = self.backend.get_mute() {
                    let _ = self.backend.set_mute(!m);
                    self.refresh_ui();
                }
            }
            MenuAction::VolEnabled => {
                self.cfg.volume_limit_enabled = !self.cfg.volume_limit_enabled;
                self.save_and_refresh(true);
            }
            MenuAction::VolLimit(v) => {
                self.cfg.volume_limit = v;
                self.cfg.volume_limit_enabled = true;
                self.save_and_refresh(true);
            }
            MenuAction::VolCustom => {
                if let Some(v) = crate::platform::prompt_custom_limit(&self.cfg.lang) {
                    self.cfg.volume_limit = v;
                    self.cfg.volume_limit_enabled = true;
                    self.save_and_refresh(true);
                }
            }
            MenuAction::WheelAccel => {
                self.cfg.wheel_acceleration = !self.cfg.wheel_acceleration;
                let _ = self.cfg.save();
                let devs = self.backend.enumerate_devices().unwrap_or_default();
                let def = self.backend.get_default_device();
                self.tray.rebuild_menu(
                    &self.cfg,
                    &devs,
                    def.as_ref().map(|d| d.id.as_str()),
                    self.backend.get_mute().unwrap_or(false),
                );
            }
            MenuAction::OpenMixer => crate::ui::open_volume_mixer(),
            MenuAction::OpenSound => crate::ui::open_sound_settings(),
            MenuAction::Autostart => {
                let new_val = !self.cfg.autostart;
                match crate::platform::set_autostart(new_val) {
                    Ok(()) => {
                        self.cfg.autostart = new_val;
                        self.save_and_refresh(false);
                    }
                    Err(_) => crate::platform::show_autostart_error(),
                }
            }
            MenuAction::LangZh => {
                self.cfg.lang = "zh".into();
                self.save_and_refresh(false);
            }
            MenuAction::LangEn => {
                self.cfg.lang = "en".into();
                self.save_and_refresh(false);
            }
            MenuAction::About => {
                let _ = crate::platform::shell::open_file(
                    "https://github.com/Wildfire2282/audio-switcher-rust",
                    None,
                );
            }
            MenuAction::Exit => std::process::exit(0),
            MenuAction::Unknown(_) => {}
        }
    }

    // ---- handlers extracted to keep `run` at ~30 lines ----
    fn pump_messages() {
        #[cfg(windows)]
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE};
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
    fn maybe_install_hook(&mut self) {
        if self.hook.is_none() && Instant::now() >= self.hook_install_at {
            self.hook = hook::WheelHook::install();
        }
    }
    fn poll_tray(&mut self) {
        let tray_rx = tray_icon::TrayIconEvent::receiver();
        let Ok(event) = tray_rx.try_recv() else { return; };
        match event {
            tray_icon::TrayIconEvent::Click { button: tray_icon::MouseButton::Middle, button_state: tray_icon::MouseButtonState::Up, .. } => {
                if let Ok(m) = self.backend.get_mute() {
                    let _ = self.backend.set_mute(!m);
                    self.refresh_ui();
                }
                self.is_hover = true;
                self.last_hover = Instant::now();
                self.wheel.clear();
            }
            tray_icon::TrayIconEvent::Enter { .. } | tray_icon::TrayIconEvent::Move { .. } | tray_icon::TrayIconEvent::Click { .. } => {
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
    fn poll_cursor(&mut self) {
        #[cfg(windows)]
        {
            let need = hook::peek_pending() || self.is_hover || self.last_hover.elapsed() < Duration::from_millis(2500);
            if need && self.last_cursor_check.elapsed() > Duration::from_millis(80) {
                self.last_cursor_check = Instant::now();
                if let Some(over) = hook::cursor_over_tray(&self.tray) {
                    self.last_cursor_over = over;
                    if over {
                        if !self.is_hover { self.is_hover = true; self.wheel.clear(); }
                        self.last_hover = Instant::now();
                    }
                }
                // rect None -> keep last_cursor_over as-is, wheel still allowed via grace
            }
        }
    }
    fn poll_menu(&mut self) {
        let menu_rx = muda::MenuEvent::receiver();
        if let Ok(event) = menu_rx.try_recv() { self.handle_menu(&event.id.0); }
    }
    fn poll_wheel(&mut self) {
        if !hook::take_pending() { return; }
        let delta = hook::take_delta();
        if !(self.is_hover || self.last_hover.elapsed() < Duration::from_millis(2500) || self.last_cursor_over) || delta == 0 { return; }
        self.last_hover = Instant::now();
        let step = self.wheel.push(Instant::now(), self.cfg.wheel_acceleration, delta);
        let total = WheelState::total_step(delta, step);
        if let Ok(vol) = self.backend.get_volume() {
            let new_vol = (vol as i32 + total).clamp(0, 100) as u32;
            let clamped = crate::config::clamp_volume(new_vol, &self.cfg);
            let _ = self.backend.set_volume(clamped);
            self.update_tooltip_and_icon();
        }
    }
    fn poll_devices(&mut self) {
        if !self.backend.poll_device_changed() {
            return;
        }
        // coalesce bursts: IMMNotificationClient may fire Added/Removed/DefaultChanged in quick succession.
        if self.last_devices_rebuild.elapsed() < Duration::from_millis(120) {
            return;
        }
        self.last_devices_rebuild = Instant::now();
        let _ = self.backend.clamp_volume_if_needed(&self.cfg);
        self.refresh_ui();
    }
    fn wait(&self) {
        #[cfg(windows)]
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{MsgWaitForMultipleObjectsEx, MWMO_INPUTAVAILABLE, QS_ALLINPUT};
            let timeout = if hook::peek_pending() { 8 } else { 500 };
            let _ = MsgWaitForMultipleObjectsEx(Some(&[]), timeout, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
        }
        #[cfg(not(windows))]
        std::thread::sleep(Duration::from_millis(if hook::peek_pending() { 8 } else { 24 }));
    }

    pub fn run(mut self) {
        loop {
            Self::pump_messages();
            self.maybe_install_hook();
            self.poll_tray();
            self.poll_cursor();
            self.poll_menu();
            self.poll_wheel();
            self.poll_devices();
            self.wait();
        }
    }
}

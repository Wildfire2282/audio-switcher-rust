//! Application entry — owns all runtime state and runs the message loop.

pub mod handler;

use std::time::{Duration, Instant};

use crate::audio::{AudioBackend, RealBackend};
use crate::config::{AppConfig, Lang};
use crate::platform::hook;
use crate::ui::{format_tooltip, TrayWrapper, WheelState};
use handler::MenuAction;

fn ensure_autostart(cfg: &AppConfig) {
    if cfg.autostart && !crate::platform::is_autostart_enabled() {
        std::thread::spawn(|| {
            let _ = crate::platform::set_autostart(true);
        });
    }
}

fn instant_ago(d: Duration) -> Instant {
    Instant::now().checked_sub(d).unwrap_or_else(Instant::now)
}

/// App owns all runtime state. Generic over [`AudioBackend`] for test injection.
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
    should_exit: bool,
    _com: crate::platform::ComGuard,
}

/// Builder for [`App`] — allows injecting a custom backend or config for tests.
///
/// # Examples
///
/// ```
/// use audio_switcher_rust::app::AppBuilder;
/// use audio_switcher_rust::ComGuard;
/// // let com = ComGuard::init().unwrap();
/// // let app = AppBuilder::new(com).build();
/// ```
pub struct AppBuilder {
    com: crate::platform::ComGuard,
    cfg: Option<AppConfig>,
}

impl AppBuilder {
    /// Create a builder with the given COM guard.
    #[must_use]
    pub fn new(com: crate::platform::ComGuard) -> Self {
        Self { com, cfg: None }
    }

    /// Override the config (otherwise loaded from disk).
    #[must_use]
    pub fn config(mut self, cfg: AppConfig) -> Self {
        self.cfg = Some(cfg);
        self
    }

    /// Build the [`App`] with the real backend.
    ///
    /// # Panics
    ///
    /// Panics if the tray icon cannot be created (e.g. Explorer not running).
    #[must_use]
    pub fn build(self) -> App<RealBackend> {
        let cfg = self.cfg.unwrap_or_else(AppConfig::load);
        let mut backend = RealBackend::new();
        let mut tray = TrayWrapper::new(&cfg, &[], None, false).unwrap_or_else(|e| {
            eprintln!("tray build failed, retrying: {e}");
            // Fallback: try once more; if still fails, panic with context.
            TrayWrapper::new(&cfg, &[], None, false).expect("tray build failed twice")
        });
        let snap = backend.fetch_snapshot_clamped(&cfg);
        let default_id = snap.default_device.as_ref().map(|d| d.id.clone());
        tray.rebuild_menu(&cfg, &snap.devices, default_id.as_deref(), snap.mute);
        tray.update_tooltip(format_tooltip(
            snap.default_device.as_ref(),
            snap.volume,
            snap.mute,
            cfg.lang,
        ));
        tray.update_icon(snap.mute);
        // Autostart side-effect only when config came from disk (not injected in tests).
        // Reuse the canonical helper to avoid duplication with with_backend.
        ensure_autostart(&cfg);
        App {
            cfg,
            backend,
            tray,
            wheel: WheelState::new(),
            is_hover: false,
            last_hover: instant_ago(Duration::from_secs(10)),
            last_cursor_check: instant_ago(Duration::from_secs(1)),
            last_cursor_over: false,
            last_devices_rebuild: instant_ago(Duration::from_secs(1)),
            hook: None,
            hook_install_at: Instant::now() + Duration::from_millis(180),
            should_exit: false,
            _com: self.com,
        }
    }
}

impl App<RealBackend> {
    /// Create a new `App` with the real Windows audio backend.
    pub fn new(com: crate::platform::ComGuard) -> Self {
        Self::with_backend(com, RealBackend::new())
    }
}

impl<B: AudioBackend> App<B> {
    /// Create an `App` with an injected backend.
    ///
    /// # Panics
    ///
    /// Panics if the tray icon cannot be created.
    pub fn with_backend(com: crate::platform::ComGuard, mut backend: B) -> Self {
        let cfg = AppConfig::load();
        ensure_autostart(&cfg);
        let mut tray = TrayWrapper::new(&cfg, &[], None, false).unwrap_or_else(|e| {
            eprintln!("tray build failed: {e}");
            TrayWrapper::new(&cfg, &[], None, false).expect("tray build retry failed")
        });
        let snap = backend.fetch_snapshot_clamped(&cfg);
        let default_id = snap.default_device.as_ref().map(|d| d.id.clone());
        tray.rebuild_menu(&cfg, &snap.devices, default_id.as_deref(), snap.mute);
        tray.update_tooltip(format_tooltip(
            snap.default_device.as_ref(),
            snap.volume,
            snap.mute,
            cfg.lang,
        ));
        tray.update_icon(snap.mute);
        Self {
            cfg,
            backend,
            tray,
            wheel: WheelState::new(),
            is_hover: false,
            last_hover: instant_ago(Duration::from_secs(10)),
            last_cursor_check: instant_ago(Duration::from_secs(1)),
            last_cursor_over: false,
            last_devices_rebuild: instant_ago(Duration::from_secs(1)),
            hook: None,
            hook_install_at: Instant::now() + Duration::from_millis(180),
            should_exit: false,
            _com: com,
        }
    }

    /// Returns true when an exit has been requested via the tray menu.
    #[must_use]
    pub fn should_exit(&self) -> bool {
        self.should_exit
    }

    fn refresh_ui(&mut self) {
        // Use batch snapshot to avoid 3 separate COM round-trips.
        let snap = self.backend.fetch_snapshot_clamped(&self.cfg);
        let def_id = snap.default_device.as_ref().map(|d| d.id.as_str());
        self.tray.rebuild_menu(&self.cfg, &snap.devices, def_id, snap.mute);
        self.tray.update_tooltip(format_tooltip(
            snap.default_device.as_ref(),
            snap.volume,
            snap.mute,
            self.cfg.lang,
        ));
        self.tray.update_icon(snap.mute);
    }

    fn update_tooltip_and_icon(&mut self) {
        // Single COM round-trip via get_volume_and_mute; fallback only on error.
        #[cfg(windows)]
        if let Ok((vol, mute)) = self.backend.get_volume_and_mute() {
            let dev = self.backend.get_default_device();
            self.tray.update_tooltip(format_tooltip(dev.as_ref(), vol, mute, self.cfg.lang));
            self.tray.update_icon(mute);
            return;
        }
        let dev = self.backend.get_default_device();
        let vol = self.backend.get_volume().unwrap_or(0);
        let mute = self.backend.get_mute().unwrap_or(false);
        self.tray.update_tooltip(format_tooltip(dev.as_ref(), vol, mute, self.cfg.lang));
        self.tray.update_icon(mute);
    }

    fn save_and_refresh(&mut self, clamp: bool) {
        // Synchronous save for critical config — avoids loss on fast exit.
        if let Err(e) = self.cfg.save_to(&AppConfig::config_path()) {
            eprintln!("config save failed: {e}");
        }
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
                    crate::platform::shell::show_error(&format!("切换设备失败: {e}"));
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
                if let Some(v) = crate::platform::prompt_custom_limit(self.cfg.lang) {
                    self.cfg.volume_limit = v;
                    self.cfg.volume_limit_enabled = true;
                    self.save_and_refresh(true);
                }
            }
            MenuAction::WheelAccel => {
                self.cfg.wheel_acceleration = !self.cfg.wheel_acceleration;
                if let Err(e) = self.cfg.save_to(&AppConfig::config_path()) {
                    eprintln!("config save failed: {e}");
                }
                // Rebuild menu only — reuse snapshot path to keep COM calls batched.
                self.refresh_ui();
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
                self.cfg.lang = Lang::Zh;
                self.save_and_refresh(false);
            }
            MenuAction::LangEn => {
                self.cfg.lang = Lang::En;
                self.save_and_refresh(false);
            }
            MenuAction::About => {
                let _ = crate::platform::shell::open_file(
                    "https://github.com/Wildfire2282/audio-switcher-rust",
                    None,
                );
            }
            MenuAction::Exit => {
                self.should_exit = true;
                #[cfg(windows)]
                unsafe {
                    use windows::Win32::UI::WindowsAndMessaging::PostQuitMessage;
                    PostQuitMessage(0);
                }
            }
            MenuAction::Unknown(s) => {
                eprintln!("unknown menu id: {s}");
            }
        }
    }

    // ---- handlers extracted to keep `run` at ~30 lines ----
    fn pump_messages() {
        #[cfg(windows)]
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{
                DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
            };
            let mut msg = MSG::default();
            // SAFETY: MSG is a plain POD out-param for PeekMessageW; &raw mut/const avoids reborrow lint
            while PeekMessageW(&raw mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&raw const msg);
                DispatchMessageW(&raw const msg);
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
        let Ok(event) = tray_rx.try_recv() else {
            return;
        };
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
            | tray_icon::TrayIconEvent::Click { .. }
            | tray_icon::TrayIconEvent::DoubleClick { .. } => {
                self.is_hover = true;
                self.last_hover = Instant::now();
                self.wheel.clear();
            }
            tray_icon::TrayIconEvent::Leave { .. } => {
                self.is_hover = false;
                self.wheel.clear();
            }
            _ => {}
        }
    }
    fn poll_cursor(&mut self) {
        #[cfg(windows)]
        {
            let need = hook::peek_pending()
                || self.is_hover
                || self.last_hover.elapsed() < Duration::from_millis(2500);
            if need && self.last_cursor_check.elapsed() > Duration::from_millis(80) {
                self.last_cursor_check = Instant::now();
                match hook::cursor_over_tray(&self.tray) {
                    Some(over) => {
                        self.last_cursor_over = over;
                        if over {
                            if !self.is_hover {
                                self.is_hover = true;
                                self.wheel.clear();
                            }
                            self.last_hover = Instant::now();
                        }
                    }
                    None => {
                        // Tray rect unavailable — treat as not over after grace period.
                        if self.last_hover.elapsed() >= Duration::from_millis(2500) {
                            self.last_cursor_over = false;
                        }
                    }
                }
            }
        }
    }
    fn poll_menu(&mut self) {
        let menu_rx = muda::MenuEvent::receiver();
        if let Ok(event) = menu_rx.try_recv() {
            self.handle_menu(&event.id.0);
        }
    }
    fn poll_wheel(&mut self) {
        if !hook::take_pending() {
            return;
        }
        let delta = hook::take_delta();
        if !(self.is_hover
            || self.last_hover.elapsed() < Duration::from_millis(2500)
            || self.last_cursor_over)
            || delta == 0
        {
            return;
        }
        self.last_hover = Instant::now();
        let step = self.wheel.push(Instant::now(), self.cfg.wheel_acceleration, delta);
        let total = WheelState::total_step(delta, step);
        if let Ok(vol) = self.backend.get_volume() {
            // vol is 0..=100; widen infallibly, clamp then convert — MSRV 1.75 has no cast_unsigned.
            let new_vol =
                u32::try_from((i32::try_from(vol).unwrap_or(0) + total).clamp(0, 100)).unwrap_or(0);
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
    // `wait` has no self data; make it an associated function (fixes clippy::unused_self)
    /// Wait for input with adaptive timeout based on wheel pending state.
    fn wait() {
        #[cfg(windows)]
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{
                MsgWaitForMultipleObjectsEx, MWMO_INPUTAVAILABLE, QS_ALLINPUT,
            };
            let timeout = if hook::peek_pending() { 8 } else { 500 };
            // SAFETY: MsgWaitForMultipleObjectsEx with empty handle slice and QS_ALLINPUT is safe to call on UI thread
            let _ =
                MsgWaitForMultipleObjectsEx(Some(&[]), timeout, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
        }
        #[cfg(not(windows))]
        std::thread::sleep(Duration::from_millis(if hook::peek_pending() { 8 } else { 24 }));
    }

    /// Run the message loop until `Exit` is requested.
    pub fn run(mut self) {
        loop {
            Self::pump_messages();
            if self.should_exit {
                break;
            }
            self.maybe_install_hook();
            self.poll_tray();
            self.poll_cursor();
            self.poll_menu();
            if self.should_exit {
                break;
            }
            self.poll_wheel();
            self.poll_devices();
            Self::wait();
        }
    }
}

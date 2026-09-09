//! Application entry — owns all runtime state and runs the message loop.
//!
//! Loop policy lives here; every Win32 call is behind `platform`
//! (`pump` for messages/wait/quit, `hook` for the wheel hook).

pub mod handler;

use std::time::{Duration, Instant};

use crate::audio::{AudioBackend, RealBackend};
use crate::config::{AppConfig, Lang};
use crate::platform::hook;
use crate::platform::{AutostartState, autostart_state, pump};
use crate::ui::tray::TrayError;
use crate::ui::{MenuState, TrayWrapper, WheelState, format_tooltip};
use handler::MenuAction;

/// Tray-build attempts at startup: Explorer may be restarting exactly then.
const TRAY_BOOT_ATTEMPTS: u32 = 3;
/// Pause between tray-build attempts (bounded: 3 × 250ms worst case).
const TRAY_BOOT_RETRY_WAIT: Duration = Duration::from_millis(250);
/// Volume-adjust arm window: a left-click on the tray icon arms wheel control
/// for this long; every wheel tick re-arms, so continuous rolling never drops
/// mid-gesture while a stale click cannot change volume minutes later.
const VOLUME_ARM_DURATION: Duration = Duration::from_secs(3);

/// Self-heal the autostart entry when the user wants it but the registry
/// reads explicit `Disabled`. `Unknown` never writes — it only logs; the
/// menu renders the item grayed.
fn ensure_autostart(cfg: &AppConfig) {
    if !cfg.autostart {
        return;
    }
    match autostart_state() {
        AutostartState::Enabled => {}
        AutostartState::Disabled => {
            std::thread::spawn(|| {
                if let Err(e) = crate::platform::set_autostart(true) {
                    crate::platform::dialog::show_autostart_error(&e);
                }
            });
        }
        AutostartState::Unknown(reason) => {
            tracing::warn!(
                "autostart state unknown at startup ({reason}); leaving registry untouched"
            );
        }
    }
}

/// Whether wheel control is currently armed (`now` is before the deadline).
fn is_armed(armed_until: Option<Instant>, now: Instant) -> bool {
    armed_until.is_some_and(|deadline| now < deadline)
}

/// App owns all runtime state. Generic over [`AudioBackend`] for test injection.
pub struct App<B: AudioBackend = RealBackend> {
    cfg: AppConfig,
    /// Effective UI language, resolved once at startup (`System` → locale).
    ui_lang: Lang,
    backend: B,
    tray: TrayWrapper,
    wheel: WheelState,
    /// Wheel-control arm deadline set by left-click; `None` means disarmed.
    /// Hover alone never arms: without a preceding left-click the wheel is ignored.
    volume_armed_until: Option<Instant>,
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
/// use audio_switcher::app::AppBuilder;
/// use audio_switcher::ComGuard;
/// // let com = ComGuard::init().expect("COM");
/// // let app = AppBuilder::new(com).build().expect("tray");
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
    /// # Errors
    ///
    /// Returns [`TrayError`] when the tray icon cannot be created; the caller
    /// dialogs and exits (a transient Explorer absence is retried inside
    #[must_use = "a failed build must dialog and exit, never be ignored"]
    pub fn build(self) -> Result<App<RealBackend>, TrayError> {
        let cfg = self.cfg.unwrap_or_else(AppConfig::load);
        App::assemble(cfg, RealBackend::new(), self.com)
    }
}
impl App<RealBackend> {
    /// Create a new `App` with the real Windows audio backend.
    #[must_use = "a failed construction must dialog and exit, never be ignored"]
    pub fn new(com: crate::platform::ComGuard) -> Result<Self, TrayError> {
        Self::with_backend(com, RealBackend::new())
    }
}

impl<B: AudioBackend> App<B> {
    /// Create an `App` with an injected backend.
    ///
    /// # Errors
    ///
    /// Returns [`TrayError`] when the tray icon cannot be created.
    pub fn with_backend(com: crate::platform::ComGuard, backend: B) -> Result<Self, TrayError> {
        let cfg = AppConfig::load();
        Self::assemble(cfg, backend, com)
    }

    /// Single assembly path shared by [`AppBuilder::build`] and
    /// [`with_backend`](Self::with_backend): tray, snapshot, and state init.
    ///
    /// The tray build retries briefly: Explorer may be restarting exactly as
    /// we start. Other failures (bad icon bytes) are deterministic, so the
    /// bound keeps a broken install from hanging startup — the surviving
    /// error propagates for a visible dialog + exit, never a panic.
    fn assemble(
        cfg: AppConfig,
        mut backend: B,
        com: crate::platform::ComGuard,
    ) -> Result<Self, TrayError> {
        ensure_autostart(&cfg);
        let ui_lang = cfg.effective_lang();
        let autostart = autostart_state();
        let boot = MenuState {
            cfg: &cfg,
            devices: &[],
            default_id: None,
            inputs: &[],
            default_input_id: None,
            muted: false,
            autostart: &autostart,
            ui_lang,
        };
        let mut last_err = None;
        let mut tray = None;
        for _ in 0..TRAY_BOOT_ATTEMPTS {
            match TrayWrapper::new(&boot) {
                Ok(built) => {
                    tray = Some(built);
                    break;
                }
                Err(e) => {
                    last_err = Some(e);
                    std::thread::sleep(TRAY_BOOT_RETRY_WAIT);
                }
            }
        }
        let mut tray =
            tray.ok_or_else(|| last_err.expect("retry loop always runs at least once"))?;
        let snap = backend.fetch_snapshot_clamped(&cfg);
        let default_id = snap.default_device.as_ref().map(|d| d.id.clone());
        let default_input_id = snap.default_input_device.as_ref().map(|d| d.id.clone());
        tray.rebuild_menu(&MenuState {
            cfg: &cfg,
            devices: &snap.devices,
            default_id: default_id.as_deref(),
            inputs: &snap.input_devices,
            default_input_id: default_input_id.as_deref(),
            muted: snap.mute,
            autostart: &autostart,
            ui_lang,
        });
        tray.update_tooltip(format_tooltip(
            snap.default_device.as_ref(),
            snap.volume,
            snap.mute,
            ui_lang,
        ));
        tray.update_icon(snap.mute);
        Ok(Self {
            cfg,
            ui_lang,
            backend,
            tray,
            wheel: WheelState::new(),
            volume_armed_until: None,
            last_devices_rebuild: Instant::now(),
            hook: None,
            hook_install_at: Instant::now() + Duration::from_millis(180),
            should_exit: false,
            _com: com,
        })
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
        let def_input_id = snap.default_input_device.as_ref().map(|d| d.id.as_str());
        let autostart = autostart_state();
        // In-place menu update; rebuilds only when the device list changed.
        self.tray.sync_menu(&MenuState {
            cfg: &self.cfg,
            devices: &snap.devices,
            default_id: def_id,
            inputs: &snap.input_devices,
            default_input_id: def_input_id,
            muted: snap.mute,
            autostart: &autostart,
            ui_lang: self.ui_lang,
        });
        self.tray.update_tooltip(format_tooltip(
            snap.default_device.as_ref(),
            snap.volume,
            snap.mute,
            self.ui_lang,
        ));
        self.tray.update_icon(snap.mute);
    }

    fn update_tooltip_and_icon(&mut self) {
        // Single COM round-trip via get_volume_and_mute; fallback only on error.
        #[cfg(windows)]
        if let Ok((vol, mute)) = self.backend.get_volume_and_mute() {
            let dev = self.backend.get_default_device();
            self.tray
                .update_tooltip(format_tooltip(dev.as_ref(), vol, mute, self.ui_lang));
            self.tray.update_icon(mute);
            return;
        }
        tracing::warn!("batch volume read failed; falling back to individual queries");
        let dev = self.backend.get_default_device();
        let vol = self.backend.get_volume().unwrap_or(0);
        let mute = self.backend.get_mute().unwrap_or(false);
        self.tray
            .update_tooltip(format_tooltip(dev.as_ref(), vol, mute, self.ui_lang));
        self.tray.update_icon(mute);
    }

    fn save_and_refresh(&mut self, clamp: bool) {
        // Synchronous save for critical config — avoids loss on fast exit.
        if let Err(e) = self.cfg.save_to(&AppConfig::config_path()) {
            tracing::warn!("config save failed: {e}");
        }
        if clamp && self.cfg.volume_limit_enabled {
            if let Err(e) = self.backend.clamp_volume_if_needed(&self.cfg) {
                tracing::warn!("volume clamp failed: {e}");
            }
        }
        self.refresh_ui();
    }

    fn lang(&self) -> Lang {
        self.ui_lang
    }

    fn handle_menu(&mut self, id: &str) {
        use crate::ui::i18n::tr;
        match MenuAction::from_id(id) {
            MenuAction::Device(dev_id) => match self.backend.set_default_device(&dev_id) {
                Ok(()) => {
                    if let Err(e) = self.backend.clamp_volume_if_needed(&self.cfg) {
                        tracing::warn!("volume clamp failed: {e}");
                    }
                    self.refresh_ui();
                }
                Err(e) => {
                    tracing::warn!("set_default_device failed: {e}");
                    crate::platform::dialog::show_msgbox(&format!(
                        "{}: {e}",
                        tr("device_error", self.lang())
                    ));
                }
            },
            MenuAction::InputDevice(dev_id) => match self.backend.set_default_input_device(&dev_id)
            {
                Ok(()) => self.refresh_ui(),
                Err(e) => {
                    tracing::warn!("set_default_input_device failed: {e}");
                    crate::platform::dialog::show_msgbox(&format!(
                        "{}: {e}",
                        tr("input_error", self.lang())
                    ));
                }
            },
            MenuAction::Mute => match self.backend.get_mute() {
                Ok(m) => {
                    if let Err(e) = self.backend.set_mute(!m) {
                        tracing::warn!("set_mute failed: {e}");
                    }
                    self.refresh_ui();
                }
                Err(e) => tracing::warn!("get_mute failed: {e}"),
            },
            MenuAction::VolEnabled => {
                self.cfg.volume_limit_enabled = !self.cfg.volume_limit_enabled;
                self.save_and_refresh(true);
            }
            MenuAction::VolLimit(v) => {
                self.cfg.volume_limit = v;
                self.cfg.volume_limit_enabled = true;
                self.save_and_refresh(true);
            }
            MenuAction::Refresh => {
                // Manual fallback for sleep-resume/callback loss: drop caches,
                // re-enumerate, and rebuild the UI from fresh state.
                self.backend.clear_cache();
                self.refresh_ui();
            }
            MenuAction::OpenMixer => {
                crate::platform::shell::open_volume_mixer(&tr("mixer_error", self.lang()));
            }
            MenuAction::OpenSound => {
                crate::platform::shell::open_sound_settings(&tr("sound_error", self.lang()));
            }
            MenuAction::Autostart => {
                let new_val = !self.cfg.autostart;
                match crate::platform::set_autostart(new_val) {
                    Ok(()) => {
                        self.cfg.autostart = new_val;
                        self.save_and_refresh(false);
                    }
                    Err(e) => {
                        tracing::warn!("set_autostart failed: {e}");
                        crate::platform::dialog::show_autostart_error(&e);
                    }
                }
            }
            MenuAction::LangSystem => {
                self.cfg.lang = Lang::System;
                self.ui_lang = self.cfg.effective_lang();
                self.save_and_refresh(false);
            }
            MenuAction::LangZh => {
                self.cfg.lang = Lang::Zh;
                self.ui_lang = Lang::Zh;
                self.save_and_refresh(false);
            }
            MenuAction::LangEn => {
                self.cfg.lang = Lang::En;
                self.ui_lang = Lang::En;
                self.save_and_refresh(false);
            }
            MenuAction::About => {
                if let Ok(url) = crate::ABOUT_URL.parse::<crate::platform::shell::Url>() {
                    crate::platform::shell::open_url(&url);
                }
            }
            MenuAction::Exit => {
                self.should_exit = true;
                pump::quit();
            }
            MenuAction::Unknown(s) => {
                // Unknown ids are a menu/handler contract breach: loud in
                // debug, logged and ignored in release (never silent).
                debug_assert!(false, "unknown menu id: {s}");
                tracing::warn!("unknown menu id ignored: {s}");
            }
        }
    }

    // ---- handlers extracted to keep `run` short ----
    fn maybe_install_hook(&mut self) {
        if self.hook.is_none() && Instant::now() >= self.hook_install_at {
            self.hook = hook::WheelHook::install();
        }
    }
    /// Arm wheel control for [`VOLUME_ARM_DURATION`], resetting acceleration
    /// so a stale burst cannot jump the volume, and refresh the tooltip so
    /// the click gives visible feedback (current device + volume).
    fn arm_volume(&mut self) {
        self.volume_armed_until = Some(Instant::now() + VOLUME_ARM_DURATION);
        self.wheel.clear();
        self.update_tooltip_and_icon();
    }
    /// Drop the arm (cursor left the icon, or the menu took over the gesture).
    fn disarm_volume(&mut self) {
        self.volume_armed_until = None;
        self.wheel.clear();
    }
    fn poll_tray(&mut self) {
        use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};
        // Drain: a burst of tray events must all be consumed each frame.
        let tray_rx = TrayIconEvent::receiver();
        while let Ok(event) = tray_rx.try_recv() {
            match event {
                TrayIconEvent::Click {
                    button: MouseButton::Middle,
                    button_state: MouseButtonState::Up,
                    ..
                } => {
                    match self.backend.get_mute() {
                        Ok(m) => {
                            if let Err(e) = self.backend.set_mute(!m) {
                                tracing::warn!(error = %e, "set_mute failed");
                            }
                            self.refresh_ui();
                        }
                        Err(e) => tracing::warn!(error = %e, "get_mute failed"),
                    }
                    // Mute never arms: volume still needs its own left-click.
                    self.wheel.clear();
                }
                // Left-click arms wheel control; the wheel only works inside
                // the arm window, so hovering or scrolling past never changes
                // the volume by accident.
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
                | TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                } => self.arm_volume(),
                // Right-click hands the gesture to the context menu: a wheel
                // roll over the open menu must scroll the menu, not the volume.
                TrayIconEvent::Click {
                    button: MouseButton::Right,
                    ..
                }
                | TrayIconEvent::DoubleClick {
                    button: MouseButton::Right,
                    ..
                }
                | TrayIconEvent::Leave { .. } => self.disarm_volume(),
                // Hover/move never arms: without a preceding left-click the
                // wheel stays ignored. Other buttons have no gesture.
                TrayIconEvent::Enter { .. }
                | TrayIconEvent::Move { .. }
                | TrayIconEvent::Click { .. }
                | TrayIconEvent::DoubleClick { .. } => {}
                // Required: `TrayIconEvent` is `#[non_exhaustive]`, so future
                // shell variants must ignore-and-continue rather than break the
                // build (same policy as the menu unknown-id path, minus the
                // warn: unknown hover/move-class events are noise).
                _ => {}
            }
        }
    }
    fn poll_menu(&mut self) {
        // Drain: rapid clicks must all dispatch each frame.
        let menu_rx = muda::MenuEvent::receiver();
        while let Ok(event) = menu_rx.try_recv() {
            self.handle_menu(&event.id.0);
        }
    }
    fn poll_wheel(&mut self) {
        let (pending, delta) = hook::take_wheel_event();
        if !pending || delta == 0 {
            return;
        }
        let now = Instant::now();
        // Gate 1: a left-click must have armed control within the window.
        // Hovering or scrolling past without clicking stays ignored.
        if !is_armed(self.volume_armed_until, now) {
            self.volume_armed_until = None;
            return;
        }
        #[cfg(windows)]
        {
            // Gate 2: the cursor must still be over the icon at event time,
            // so a click followed by scrolling elsewhere cannot change
            // the volume. Fail closed when the rect is unavailable.
            if !hook::cursor_over_tray(&self.tray).unwrap_or(false) {
                return;
            }
        }
        // Sliding window: rolling keeps control; idling past the window
        // needs a fresh left-click.
        self.volume_armed_until = Some(now + VOLUME_ARM_DURATION);
        let step = self.wheel.push(now, delta);
        let total = WheelState::total_step(delta, step);
        match self.backend.get_volume() {
            Ok(vol) => {
                // vol is 0..=100; widen infallibly, clamp then convert.
                let new_vol =
                    u32::try_from((i32::try_from(vol).unwrap_or(0) + total).clamp(0, 100))
                        .unwrap_or(0);
                let clamped = crate::config::clamp_volume(new_vol, &self.cfg);
                if let Err(e) = self.backend.set_volume(clamped) {
                    tracing::warn!(error = %e, "set_volume failed");
                }
                self.update_tooltip_and_icon();
            }
            Err(e) => tracing::warn!(error = %e, "get_volume failed"),
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
        if let Err(e) = self.backend.clamp_volume_if_needed(&self.cfg) {
            tracing::warn!("volume clamp failed: {e}");
        }
        self.refresh_ui();
    }
    /// External volume/mute change (media keys, other apps) — refresh tooltip
    /// and icon without touching the menu.
    fn poll_volume_state(&mut self) {
        if self.backend.take_volume_changed() {
            self.update_tooltip_and_icon();
        }
    }

    /// Run the message loop until `Exit` is requested.
    pub fn run(mut self) {
        loop {
            pump::pump_messages();
            if self.should_exit {
                break;
            }
            self.maybe_install_hook();
            self.poll_tray();
            self.poll_menu();
            if self.should_exit {
                break;
            }
            self.poll_wheel();
            self.poll_devices();
            self.poll_volume_state();
            pump::wait_for_input(hook::peek_pending());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_requires_left_click_arm() {
        let now = Instant::now();
        // Hover alone never arms: disarmed stays disarmed.
        assert!(!is_armed(None, now));
        // A fresh click arms.
        let armed = Some(now + VOLUME_ARM_DURATION);
        assert!(is_armed(armed, now));
        // A stale click expires: scrolling minutes later stays ignored.
        assert!(!is_armed(armed, now + VOLUME_ARM_DURATION));
        assert!(!is_armed(
            armed,
            now + VOLUME_ARM_DURATION + Duration::from_millis(1)
        ));
    }

    #[test]
    fn arm_window_is_short_and_sliding() {
        // Short enough that a forgotten click cannot surprise later, long
        // enough to roll a full gesture; each tick re-arms from `now`.
        assert!(VOLUME_ARM_DURATION <= Duration::from_secs(5));
        let now = Instant::now();
        let extended = Some(now + VOLUME_ARM_DURATION);
        assert!(is_armed(extended, now));
    }
}

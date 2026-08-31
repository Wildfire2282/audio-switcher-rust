#![windows_subsystem = "windows"]

mod audio;
mod config;
mod system;
mod tray;

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use audio::{AudioBackend, RealBackend};
use config::AppConfig;

#[cfg(windows)]
use windows::core::PCWSTR;
#[cfg(windows)]
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};

static IS_HOVER: AtomicBool = AtomicBool::new(false);
static WHEEL_DELTA: AtomicI32 = AtomicI32::new(0);
static WHEEL_PENDING: AtomicBool = AtomicBool::new(false);
#[cfg(windows)]
static HOOK_HANDLE: AtomicUsize = AtomicUsize::new(0);

#[cfg(windows)]
unsafe extern "system" fn hook_proc(
    n_code: i32,
    w_param: windows::Win32::Foundation::WPARAM,
    l_param: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, MOUSEHOOKSTRUCTEX, WM_MOUSEWHEEL,
    };
    if n_code >= 0 && w_param.0 as u32 == WM_MOUSEWHEEL {
        let info = &*(l_param.0 as *const MOUSEHOOKSTRUCTEX);
        let delta = (info.mouseData >> 16) as u16 as i16 as i32;
        WHEEL_DELTA.store(delta, Ordering::SeqCst);
        WHEEL_PENDING.store(true, Ordering::SeqCst);
    }
    CallNextHookEx(None, n_code, w_param, l_param)
}

#[cfg(windows)]
fn install_wheel_hook() {
    if HOOK_HANDLE.load(Ordering::SeqCst) != 0 {
        return;
    }
    unsafe {
        use windows::Win32::System::LibraryLoader::GetModuleHandleW;
        use windows::Win32::UI::WindowsAndMessaging::{SetWindowsHookExW, WH_MOUSE_LL};
        let hinst = GetModuleHandleW(PCWSTR::null())
            .ok()
            .map(|h| windows::Win32::Foundation::HINSTANCE(h.0));
        if let Ok(hook) = SetWindowsHookExW(WH_MOUSE_LL, Some(hook_proc), hinst, 0) {
            HOOK_HANDLE.store(hook.0 as usize, Ordering::SeqCst);
        }
    }
}

#[cfg(windows)]
fn uninstall_wheel_hook() {
    let raw = HOOK_HANDLE.swap(0, Ordering::SeqCst);
    if raw != 0 {
        unsafe {
            let hook = windows::Win32::UI::WindowsAndMessaging::HHOOK(raw as *mut std::ffi::c_void);
            let _ = windows::Win32::UI::WindowsAndMessaging::UnhookWindowsHookEx(hook);
        }
    }
}

fn main() {
    let _guard = match system::SingleInstanceGuard::new("audio-switcher-rust-single-instance-v1") {
        Some(g) => g,
        None => return,
    };

    let mut cfg = AppConfig::load();
    if cfg.autostart && !system::is_autostart_enabled() {
        let _ = system::set_autostart(true);
    }

    tray::log_verbose(&cfg, "AudioSwitcher started");

    #[cfg(windows)]
    unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if hr.is_err() {
            use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONWARNING, MB_OK};
            let msg: Vec<u16> = "COM 初始化失败，程序将退出\0".encode_utf16().collect();
            let title: Vec<u16> = "Audio Switcher\0".encode_utf16().collect();
            MessageBoxW(
                None,
                PCWSTR(msg.as_ptr()),
                PCWSTR(title.as_ptr()),
                MB_OK | MB_ICONWARNING,
            );
            return;
        }
    }

    let mut backend = RealBackend::new();
    let _ = backend.clamp_volume_if_needed(&cfg);

    let devices = backend.enumerate_devices().unwrap_or_default();
    let default_dev = backend.get_default_device();
    let default_id = default_dev.as_ref().map(|d| d.id.as_str());
    let mute = backend.get_mute().unwrap_or(false);

    let mut tray_wrapper = tray::TrayWrapper::new(&cfg, &devices, default_id, mute);
    update_tooltip(&tray_wrapper, &backend, &cfg);

    let menu_rx = muda::MenuEvent::receiver();
    let tray_rx = tray_icon::TrayIconEvent::receiver();

    let mut wheel_state = tray::WheelState::new();
    let mut last_dark = tray::is_dark_mode();
    let mut last_theme_check = Instant::now();

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

        if last_theme_check.elapsed() > Duration::from_millis(500) {
            last_theme_check = Instant::now();
            let dark = tray::is_dark_mode();
            if dark != last_dark {
                last_dark = dark;
                if let Ok(m) = backend.get_mute() {
                    tray_wrapper.update_icon(m);
                }
            }
        }

        if let Ok(event) = tray_rx.try_recv() {
            match event {
                tray_icon::TrayIconEvent::Click {
                    button: tray_icon::MouseButton::Middle,
                    button_state: tray_icon::MouseButtonState::Up,
                    ..
                } => {
                    if let Ok(m) = backend.get_mute() {
                        let _ = backend.set_mute(!m);
                        let new_mute = backend.get_mute().unwrap_or(!m);
                        tray_wrapper.update_icon(new_mute);
                        update_tooltip(&tray_wrapper, &backend, &cfg);
                        let devs = backend.enumerate_devices().unwrap_or_default();
                        tray_wrapper.rebuild_menu(
                            &cfg,
                            &devs,
                            backend.get_default_device().as_ref().map(|d| d.id.as_str()),
                            new_mute,
                        );
                        tray::log_verbose(&cfg, &format!("middle mute toggle -> {}", new_mute));
                    }
                }
                tray_icon::TrayIconEvent::Enter { .. } => {
                    IS_HOVER.store(true, Ordering::SeqCst);
                    wheel_state.clear();
                    #[cfg(windows)]
                    install_wheel_hook();
                }
                tray_icon::TrayIconEvent::Leave { .. } => {
                    IS_HOVER.store(false, Ordering::SeqCst);
                    wheel_state.clear();
                    #[cfg(windows)]
                    uninstall_wheel_hook();
                }
                _ => {}
            }
        }

        if let Ok(event) = menu_rx.try_recv() {
            let id = event.id.0.clone();
            if let Some(dev_id) = id.strip_prefix("device_") {
                match backend.set_default_device(dev_id) {
                    Ok(()) => {
                        let _ = backend.clamp_volume_if_needed(&cfg);
                        tray::log_verbose(&cfg, &format!("switch to {}", dev_id));
                        let devs = backend.enumerate_devices().unwrap_or_default();
                        let def = backend.get_default_device();
                        let def_id = def.as_ref().map(|d| d.id.as_str());
                        tray_wrapper.rebuild_menu(
                            &cfg,
                            &devs,
                            def_id,
                            backend.get_mute().unwrap_or(false),
                        );
                        update_tooltip(&tray_wrapper, &backend, &cfg);
                    }
                    Err(e) => {
                        #[cfg(windows)]
                        unsafe {
                            let msg = format!("切换设备失败: {}", e);
                            let wide: Vec<u16> =
                                msg.encode_utf16().chain(std::iter::once(0)).collect();
                            let title: Vec<u16> = "Audio Switcher\0".encode_utf16().collect();
                            windows::Win32::UI::WindowsAndMessaging::MessageBoxW(
                                None,
                                PCWSTR(wide.as_ptr()),
                                PCWSTR(title.as_ptr()),
                                windows::Win32::UI::WindowsAndMessaging::MB_OK
                                    | windows::Win32::UI::WindowsAndMessaging::MB_ICONWARNING,
                            );
                        }
                    }
                }
            } else {
                match id.as_str() {
                    "mute" => {
                        if let Ok(m) = backend.get_mute() {
                            let _ = backend.set_mute(!m);
                            let new_mute = backend.get_mute().unwrap_or(!m);
                            tray_wrapper.update_icon(new_mute);
                            update_tooltip(&tray_wrapper, &backend, &cfg);
                            let devs = backend.enumerate_devices().unwrap_or_default();
                            tray_wrapper.rebuild_menu(
                                &cfg,
                                &devs,
                                backend.get_default_device().as_ref().map(|d| d.id.as_str()),
                                new_mute,
                            );
                        }
                    }
                    "vol_enabled" => {
                        cfg.volume_limit_enabled = !cfg.volume_limit_enabled;
                        let _ = cfg.save();
                        if cfg.volume_limit_enabled {
                            let _ = backend.clamp_volume_if_needed(&cfg);
                        }
                        let devs = backend.enumerate_devices().unwrap_or_default();
                        tray_wrapper.rebuild_menu(
                            &cfg,
                            &devs,
                            backend.get_default_device().as_ref().map(|d| d.id.as_str()),
                            backend.get_mute().unwrap_or(false),
                        );
                        update_tooltip(&tray_wrapper, &backend, &cfg);
                    }
                    "vol_25" => {
                        cfg.volume_limit = 25;
                        cfg.volume_limit_enabled = true;
                        let _ = cfg.save();
                        let _ = backend.clamp_volume_if_needed(&cfg);
                        let devs = backend.enumerate_devices().unwrap_or_default();
                        tray_wrapper.rebuild_menu(
                            &cfg,
                            &devs,
                            backend.get_default_device().as_ref().map(|d| d.id.as_str()),
                            backend.get_mute().unwrap_or(false),
                        );
                        update_tooltip(&tray_wrapper, &backend, &cfg);
                    }
                    "vol_50" => {
                        cfg.volume_limit = 50;
                        cfg.volume_limit_enabled = true;
                        let _ = cfg.save();
                        let _ = backend.clamp_volume_if_needed(&cfg);
                        let devs = backend.enumerate_devices().unwrap_or_default();
                        tray_wrapper.rebuild_menu(
                            &cfg,
                            &devs,
                            backend.get_default_device().as_ref().map(|d| d.id.as_str()),
                            backend.get_mute().unwrap_or(false),
                        );
                        update_tooltip(&tray_wrapper, &backend, &cfg);
                    }
                    "vol_custom" => {
                        if let Some(v) = tray::prompt_custom_limit(&cfg.lang) {
                            cfg.volume_limit = v;
                            cfg.volume_limit_enabled = true;
                            let _ = cfg.save();
                            let _ = backend.clamp_volume_if_needed(&cfg);
                            let devs = backend.enumerate_devices().unwrap_or_default();
                            tray_wrapper.rebuild_menu(
                                &cfg,
                                &devs,
                                backend.get_default_device().as_ref().map(|d| d.id.as_str()),
                                backend.get_mute().unwrap_or(false),
                            );
                            update_tooltip(&tray_wrapper, &backend, &cfg);
                        }
                    }
                    "wheel_accel" => {
                        cfg.wheel_acceleration = !cfg.wheel_acceleration;
                        let _ = cfg.save();
                        let devs = backend.enumerate_devices().unwrap_or_default();
                        tray_wrapper.rebuild_menu(
                            &cfg,
                            &devs,
                            backend.get_default_device().as_ref().map(|d| d.id.as_str()),
                            backend.get_mute().unwrap_or(false),
                        );
                    }
                    "verbose_log" => {
                        cfg.verbose_log = !cfg.verbose_log;
                        let _ = cfg.save();
                        tray::log_verbose(&cfg, &format!("verbose_log -> {}", cfg.verbose_log));
                        let devs = backend.enumerate_devices().unwrap_or_default();
                        tray_wrapper.rebuild_menu(
                            &cfg,
                            &devs,
                            backend.get_default_device().as_ref().map(|d| d.id.as_str()),
                            backend.get_mute().unwrap_or(false),
                        );
                    }
                    "open_mixer" => tray::open_volume_mixer(),
                    "open_sound" => tray::open_sound_settings(),
                    "autostart" => {
                        let new_val = !cfg.autostart;
                        match system::set_autostart(new_val) {
                            Ok(()) => {
                                cfg.autostart = new_val;
                                let _ = cfg.save();
                                let devs = backend.enumerate_devices().unwrap_or_default();
                                tray_wrapper.rebuild_menu(
                                    &cfg,
                                    &devs,
                                    backend.get_default_device().as_ref().map(|d| d.id.as_str()),
                                    backend.get_mute().unwrap_or(false),
                                );
                            }
                            Err(_) => system::show_autostart_error(),
                        }
                    }
                    "lang_zh" => {
                        cfg.lang = "zh".to_string();
                        let _ = cfg.save();
                        let devs = backend.enumerate_devices().unwrap_or_default();
                        tray_wrapper.rebuild_menu(
                            &cfg,
                            &devs,
                            backend.get_default_device().as_ref().map(|d| d.id.as_str()),
                            backend.get_mute().unwrap_or(false),
                        );
                        update_tooltip(&tray_wrapper, &backend, &cfg);
                    }
                    "lang_en" => {
                        cfg.lang = "en".to_string();
                        let _ = cfg.save();
                        let devs = backend.enumerate_devices().unwrap_or_default();
                        tray_wrapper.rebuild_menu(
                            &cfg,
                            &devs,
                            backend.get_default_device().as_ref().map(|d| d.id.as_str()),
                            backend.get_mute().unwrap_or(false),
                        );
                        update_tooltip(&tray_wrapper, &backend, &cfg);
                    }
                    "help" => tray::show_help(&cfg.lang),
                    "about" => tray::show_about(&cfg.lang),
                    "exit" => {
                        #[cfg(windows)]
                        uninstall_wheel_hook();
                        #[cfg(windows)]
                        unsafe {
                            CoUninitialize();
                        }
                        std::process::exit(0);
                    }
                    _ => {}
                }
            }
        }

        if WHEEL_PENDING.swap(false, Ordering::SeqCst) {
            let delta = WHEEL_DELTA.swap(0, Ordering::SeqCst);
            if IS_HOVER.load(Ordering::SeqCst) && delta != 0 {
                let now = Instant::now();
                let step = wheel_state.push(now, cfg.wheel_acceleration, delta);
                let total = tray::WheelState::total_step(delta, step);
                if let Ok(vol) = backend.get_volume() {
                    let new_vol = (vol as i32 + total).clamp(0, 100) as u32;
                    let clamped = config::clamp_volume(new_vol, &cfg);
                    let _ = backend.set_volume(clamped);
                    tray::log_verbose(
                        &cfg,
                        &format!(
                            "wheel delta {delta} step {step} total {total} vol {vol}->{clamped}"
                        ),
                    );
                    update_tooltip(&tray_wrapper, &backend, &cfg);
                }
            }
        }

        if backend.poll_device_changed() || audio::take_device_changed() {
            let _ = backend.clamp_volume_if_needed(&cfg);
            let devs = backend.enumerate_devices().unwrap_or_default();
            let def = backend.get_default_device();
            tray_wrapper.rebuild_menu(
                &cfg,
                &devs,
                def.as_ref().map(|d| d.id.as_str()),
                backend.get_mute().unwrap_or(false),
            );
            update_tooltip(&tray_wrapper, &backend, &cfg);
            tray::log_verbose(&cfg, "device change detected, menu rebuilt");
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}

fn update_tooltip(wrapper: &tray::TrayWrapper, backend: &RealBackend, cfg: &AppConfig) {
    let dev = backend.get_default_device();
    let vol = backend.get_volume().unwrap_or(0);
    let mute = backend.get_mute().unwrap_or(false);
    let tip = tray::format_tooltip(dev.as_ref(), vol, mute, &cfg.lang);
    wrapper.update_tooltip(tip);
    wrapper.update_icon(mute);
}

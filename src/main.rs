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
        CallNextHookEx, MSLLHOOKSTRUCT, WM_MOUSEWHEEL,
    };
    if n_code >= 0 && w_param.0 as u32 == WM_MOUSEWHEEL {
        let info = &*(l_param.0 as *const MSLLHOOKSTRUCT);
        let delta = (info.mouseData >> 16) as u16 as i16 as i32;
        // Relaxed 足够：仅标志+累加，无跨线程同步语义需求，最低栅栏开销
        WHEEL_DELTA.fetch_add(delta, Ordering::Relaxed);
        WHEEL_PENDING.store(true, Ordering::Relaxed);
    }
    CallNextHookEx(None, n_code, w_param, l_param)
}

#[cfg(windows)]
fn install_wheel_hook() {
    if HOOK_HANDLE.load(Ordering::Relaxed) != 0 {
        return;
    }
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{SetWindowsHookExW, WH_MOUSE_LL};
        if let Ok(hook) = SetWindowsHookExW(WH_MOUSE_LL, Some(hook_proc), None, 0) {
            HOOK_HANDLE.store(hook.0 as usize, Ordering::Relaxed);
        }
    }
}

#[cfg(windows)]
fn uninstall_wheel_hook() {
    let raw = HOOK_HANDLE.swap(0, Ordering::Relaxed);
    if raw != 0 {
        unsafe {
            let hook = windows::Win32::UI::WindowsAndMessaging::HHOOK(raw as *mut std::ffi::c_void);
            let _ = windows::Win32::UI::WindowsAndMessaging::UnhookWindowsHookEx(hook);
        }
    }
}

#[cfg(windows)]
fn cursor_over_tray(wrapper: &tray::TrayWrapper) -> bool {
    unsafe {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
        let mut pt = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut pt).is_err() {
            return false;
        }
        if let Some(rect) = wrapper.tray.rect() {
            // Shell_NotifyIconGetRect 在不同 DPI/任务栏模式下可能给出偏小或偏移的矩形，
            // 且托盘图标在溢出区时 rect 指向溢出按钮而非图标本身。
            // 放宽 16px 容差并对 None 兜底为 true（避免 rect 获取失败时误判为未悬停）
            let x = pt.x as f64;
            let y = pt.y as f64;
            let pad = 16.0;
            x >= rect.position.x - pad
                && x < rect.position.x + rect.size.width as f64 + pad
                && y >= rect.position.y - pad
                && y < rect.position.y + rect.size.height as f64 + pad
        } else {
            // rect 获取失败时不阻断滚轮，交由 IS_HOVER/优雅期判定
            true
        }
    }
}
#[cfg(not(windows))]
fn cursor_over_tray(_wrapper: &tray::TrayWrapper) -> bool {
    false
}

fn main() {
    let _guard = match system::SingleInstanceGuard::new("audio-switcher-rust-single-instance-v1") {
        Some(g) => g,
        None => return,
    };

    let mut cfg = AppConfig::load();
    // 非关键的开机自启检查放到后台线程，避免阻塞首帧托盘弹出
    let cfg_autostart = cfg.autostart;
    std::thread::spawn(move || {
        if cfg_autostart && !system::is_autostart_enabled() {
            let _ = system::set_autostart(true);
        }
    });

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

    // 极速首帧：先用占位托盘（默认图标，无 COM）让 Shell 立即显示，再后台批量拉音频状态
    // 避免 COM 枚举阻塞首帧 80-150ms
    let mut tray_wrapper = {
        tray::TrayWrapper::new(&cfg, &[], None, false)
    };
    // 同步批量拉取但已在托盘创建后，用户感知启动 <50ms；真正的 COM 仍在主线程但不堵首帧
    let mut backend = RealBackend::new();
    // 用合并限幅的快照，单次 Activate 内完成 volume/mute+clamp，避免二次 COM
    let snap = backend.fetch_snapshot_clamped(&cfg);
    let default_id_owned = snap.default_device.as_ref().map(|d| d.id.clone());
    {
        tray_wrapper.rebuild_menu(
            &cfg,
            &snap.devices,
            default_id_owned.as_deref(),
            snap.mute,
        );
        let tip = tray::format_tooltip(
            snap.default_device.as_ref(),
            snap.volume,
            snap.mute,
            &cfg.lang,
        );
        tray_wrapper.update_tooltip(tip);
        tray_wrapper.update_icon(snap.mute);
    }

    let menu_rx = muda::MenuEvent::receiver();
    let tray_rx = tray_icon::TrayIconEvent::receiver();

    let mut wheel_state = tray::WheelState::new();
    let mut last_dark = tray::is_dark_mode();
    let mut last_theme_check = Instant::now();
    let mut last_hover_instant = Instant::now() - Duration::from_secs(10);
    let mut last_cursor_check = Instant::now() - Duration::from_secs(1);
    let mut last_cursor_over = false;
    #[cfg(windows)]
    let hook_install_at = Instant::now() + Duration::from_millis(180);

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

        // 延迟安装钩子于主线程（WH_MOUSE_LL 必须在带消息泵的线程安装，否则不触发）
        #[cfg(windows)]
        if HOOK_HANDLE.load(Ordering::Relaxed) == 0 && Instant::now() >= hook_install_at {
            install_wheel_hook();
        }

        // 主题检测：事件驱动 TTL 2s，缓存层已 2000ms 无锁，避免高频注册表
        // 实际轮询仅作为兜底，精确唤醒由下面的 MsgWait 动态 timeout 驱动
        if last_theme_check.elapsed() > Duration::from_millis(1800) {
            last_theme_check = Instant::now();
            let dark = tray::is_dark_mode();
            if dark != last_dark {
                last_dark = dark;
                #[cfg(windows)]
                tray::invalidate_dark_mode_cache();
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
                    }
                    // 中键也视为悬停，保持滚轮可用
                    IS_HOVER.store(true, Ordering::Relaxed);
                    last_hover_instant = Instant::now();
                    wheel_state.clear();
                }
                tray_icon::TrayIconEvent::Enter { .. }
                | tray_icon::TrayIconEvent::Move { .. }
                | tray_icon::TrayIconEvent::Click { .. } => {
                    IS_HOVER.store(true, Ordering::Relaxed);
                    last_hover_instant = Instant::now();
                    wheel_state.clear();
                }
                tray_icon::TrayIconEvent::Leave { .. } => {
                    IS_HOVER.store(false, Ordering::Relaxed);
                    wheel_state.clear();
                }
                tray_icon::TrayIconEvent::DoubleClick { .. } => {
                    IS_HOVER.store(true, Ordering::Relaxed);
                    last_hover_instant = Instant::now();
                    wheel_state.clear();
                }
                _ => {}
            }
        }

        // 轮询兜底节流：仅在需要时（悬停幽灵期或有滚轮待处理）才做矩形检测，间隔 80ms
        #[cfg(windows)]
        {
            let need_cursor = WHEEL_PENDING.load(Ordering::Relaxed)
                || IS_HOVER.load(Ordering::Relaxed)
                || last_hover_instant.elapsed() < Duration::from_millis(2500);
            if need_cursor && last_cursor_check.elapsed() > Duration::from_millis(80) {
                last_cursor_check = Instant::now();
                let over = cursor_over_tray(&tray_wrapper);
                last_cursor_over = over;
                if over {
                    if !IS_HOVER.load(Ordering::Relaxed) {
                        IS_HOVER.store(true, Ordering::Relaxed);
                        wheel_state.clear();
                    }
                    last_hover_instant = Instant::now();
                }
            }
        }

        if let Ok(event) = menu_rx.try_recv() {
            let id = event.id.0.clone();
            if let Some(dev_id) = id.strip_prefix("device_") {
                match backend.set_default_device(dev_id) {
                    Ok(()) => {
                        let _ = backend.clamp_volume_if_needed(&cfg);
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

        if WHEEL_PENDING.swap(false, Ordering::Relaxed) {
            let delta = WHEEL_DELTA.swap(0, Ordering::Relaxed);
            if (IS_HOVER.load(Ordering::Relaxed)
                || last_hover_instant.elapsed() < Duration::from_millis(2500)
                || last_cursor_over)
                && delta != 0
            {
                last_hover_instant = Instant::now();
                let now = Instant::now();
                let step = wheel_state.push(now, cfg.wheel_acceleration, delta);
                let total = tray::WheelState::total_step(delta, step);
                if let Ok(vol) = backend.get_volume() {
                    let new_vol = (vol as i32 + total).clamp(0, 100) as u32;
                    let clamped = config::clamp_volume(new_vol, &cfg);
                    let _ = backend.set_volume(clamped);
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
        }

        // 事件驱动休眠：空闲时接近零唤醒，输入/设备事件立即唤醒
        #[cfg(windows)]
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{
                MsgWaitForMultipleObjectsEx, MWMO_INPUTAVAILABLE, QS_ALLINPUT,
            };
            let has_pending = WHEEL_PENDING.load(Ordering::Relaxed);
            if has_pending {
                // 快速路径 8ms
                let _ = MsgWaitForMultipleObjectsEx(
                    Some(&[]),
                    8,
                    QS_ALLINPUT,
                    MWMO_INPUTAVAILABLE,
                );
            } else {
                // 空闲时最长 500ms 醒一次处理主题/设备轮询，输入到来 MsgWait 立即返回，无卡顿
                let remaining = last_theme_check
                    .checked_add(Duration::from_millis(1800))
                    .and_then(|t| t.checked_duration_since(Instant::now()))
                    .map(|d| d.as_millis() as u32)
                    .unwrap_or(0)
                    .min(500);
                let timeout = remaining.max(8);
                let _ = MsgWaitForMultipleObjectsEx(
                    Some(&[]),
                    timeout,
                    QS_ALLINPUT,
                    MWMO_INPUTAVAILABLE,
                );
            }
        }
        #[cfg(not(windows))]
        {
            let has_pending = WHEEL_PENDING.load(Ordering::Relaxed);
            if has_pending {
                std::thread::sleep(Duration::from_millis(8));
            } else {
                // 非 windows 无钩子卡顿顾虑，可适当拉长到 32ms
                std::thread::sleep(Duration::from_millis(24));
            }
        }
    }
}

fn update_tooltip(wrapper: &tray::TrayWrapper, backend: &RealBackend, cfg: &AppConfig) {
    #[cfg(windows)]
    {
        // 批量取音量+静音，减少一次 COM 往返
        if let Ok((vol, mute)) = backend.get_volume_and_mute() {
            let dev = backend.get_default_device();
            let tip = tray::format_tooltip(dev.as_ref(), vol, mute, &cfg.lang);
            wrapper.update_tooltip(tip);
            wrapper.update_icon(mute);
            return;
        }
    }
    let dev = backend.get_default_device();
    let vol = backend.get_volume().unwrap_or(0);
    let mute = backend.get_mute().unwrap_or(false);
    let tip = tray::format_tooltip(dev.as_ref(), vol, mute, &cfg.lang);
    wrapper.update_tooltip(tip);
    wrapper.update_icon(mute);
}

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use muda::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::audio::AudioDevice;
use crate::config::AppConfig;

#[cfg(windows)]
use windows::core::PCWSTR;
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, MB_ICONWARNING, MB_OK,
};

// ------------------------------------------------------------
// i18n
// ------------------------------------------------------------
pub fn tr(key: &str, lang: &str) -> String {
    let zh = lang == "zh";
    match key {
        "mute" => {
            if zh {
                "全局静音".into()
            } else {
                "Mute".into()
            }
        }
        "volume_limit" => {
            if zh {
                "音量上限".into()
            } else {
                "Volume Limit".into()
            }
        }
        "experimental" => {
            if zh {
                "实验性功能".into()
            } else {
                "Experimental".into()
            }
        }
        "enabled" => {
            if zh {
                "启用".into()
            } else {
                "Enabled".into()
            }
        }
        "custom" => {
            if zh {
                "自定义...".into()
            } else {
                "Custom...".into()
            }
        }
        "wheel_accel" => {
            if zh {
                "滚轮加速".into()
            } else {
                "Wheel Acceleration".into()
            }
        }
        "verbose_log" => {
            if zh {
                "详细日志".into()
            } else {
                "Verbose Log".into()
            }
        }
        "open_mixer" => {
            if zh {
                "打开音量合成器".into()
            } else {
                "Open Volume Mixer".into()
            }
        }
        "open_sound" => {
            if zh {
                "打开声音设置".into()
            } else {
                "Open Sound Settings".into()
            }
        }
        "autostart" => {
            if zh {
                "开机自启".into()
            } else {
                "Auto Launch".into()
            }
        }
        "about" => {
            if zh {
                "关于".into()
            } else {
                "About".into()
            }
        }
        "exit" => {
            if zh {
                "退出".into()
            } else {
                "Exit".into()
            }
        }
        "chinese" => "中文".into(),
        "english" => "English".into(),
        "muted" => {
            if zh {
                "静音".into()
            } else {
                "Muted".into()
            }
        }
        "invalid_custom" => {
            if zh {
                "请输入 1-100 的整数".into()
            } else {
                "Please enter integer 1-100".into()
            }
        }
        "about_text" => {
            if zh {
                "Audio Switcher — 托盘音频切换工具\n纯 Rust 托盘工具\n\n右键菜单切换设备，中键静音，悬停滚轮调音量。"
                    .into()
            } else {
                "Audio Switcher — Tray audio switcher\nPure Rust tray tool\n\nRight-click to switch device, middle-click to mute, hover+wheel to adjust volume.".into()
            }
        }
        _ => key.to_string(),
    }
}

// ------------------------------------------------------------
// tooltip
// ------------------------------------------------------------
pub fn format_tooltip(device: Option<&AudioDevice>, volume: u32, mute: bool, lang: &str) -> String {
    if mute {
        tr("muted", lang)
    } else if let Some(d) = device {
        let base = format!("{} - {}%", d.name, volume);
        truncate_tooltip(&base)
    } else {
        format!("{}%", volume)
    }
}

fn truncate_tooltip(s: &str) -> String {
    if s.chars().count() > 60 {
        let truncated: String = s.chars().take(58).collect();
        format!("{}…", truncated)
    } else {
        s.to_string()
    }
}

// ------------------------------------------------------------
// wheel acceleration
// ------------------------------------------------------------
#[derive(Debug, Default)]
pub struct WheelState {
    history: VecDeque<Instant>,
}

impl WheelState {
    pub fn new() -> Self {
        Self {
            history: VecDeque::new(),
        }
    }

    /// push a wheel tick at now, returns step percent (1,2,5)
    /// Spec: 200ms window >=3格 2%，>=5格或 <80ms 5%，delta/120叠加，上限5%/格
    /// ticks = delta/120 至少1，历史按tick计数
    pub fn push(&mut self, now: Instant, wheel_accel: bool, delta: i32) -> u32 {
        if !wheel_accel {
            return 1;
        }
        let ticks = (delta.abs() / 120).max(1) as usize;
        // clean >200ms
        while let Some(front) = self.history.front() {
            if now.duration_since(*front) > Duration::from_millis(200) {
                self.history.pop_front();
            } else {
                break;
            }
        }
        // push ticks times same instant (or with 0 interval)
        for _ in 0..ticks {
            self.history.push_back(now);
        }
        let count = self.history.len();
        // interval of last two ticks
        let last_interval = if count >= 2 {
            self.history[count - 1].duration_since(self.history[count - 2])
        } else {
            Duration::from_millis(200)
        };
        calc_wheel_step(count, last_interval.as_millis(), wheel_accel)
    }

    /// compute total delta percent for given delta (may be >120) and step
    pub fn total_step(delta: i32, step_per_tick: u32) -> i32 {
        let ticks = delta / 120;
        if ticks == 0 {
            if delta > 0 {
                step_per_tick as i32
            } else {
                -(step_per_tick as i32)
            }
        } else {
            ticks * step_per_tick as i32
        }
    }

    pub fn clear(&mut self) {
        self.history.clear();
    }
}

/// Pure function for tests without Instant
pub fn calc_wheel_step(count: usize, min_interval_ms: u128, wheel_accel: bool) -> u32 {
    if !wheel_accel {
        return 1;
    }
    if count >= 5 || min_interval_ms < 80 {
        5
    } else if count >= 3 {
        2
    } else {
        1
    }
}

// ------------------------------------------------------------
// icon creation — emoji temporary substitute
// ------------------------------------------------------------
pub fn make_icon(muted: bool, is_dark: bool) -> Icon {
    #[cfg(windows)]
    if let Some(icon) = try_make_emoji_icon(muted, is_dark) {
        return icon;
    }
    // fallback simple colored circle (kept for non-windows/tests)
    let mut rgba = vec![0u8; 32 * 32 * 4];
    let (r, g, b) = if muted {
        (220, 50, 50)
    } else if is_dark {
        (80, 200, 120)
    } else {
        (40, 120, 220)
    };
    for y in 0..32 {
        for x in 0..32 {
            let dx = x as i32 - 16;
            let dy = y as i32 - 16;
            let dist = ((dx * dx + dy * dy) as f32).sqrt();
            let idx = (y * 32 + x) * 4;
            if dist < 14.0 {
                rgba[idx] = r;
                rgba[idx + 1] = g;
                rgba[idx + 2] = b;
                rgba[idx + 3] = 255;
                if muted && (x as i32 - y as i32).abs() < 2 {
                    rgba[idx] = 255;
                    rgba[idx + 1] = 255;
                    rgba[idx + 2] = 255;
                }
            } else {
                rgba[idx + 3] = 0;
            }
        }
    }
    Icon::from_rgba(rgba, 32, 32).unwrap()
}

#[cfg(windows)]
fn try_make_emoji_icon(muted: bool, is_dark: bool) -> Option<Icon> {
    use windows::Win32::Foundation::{COLORREF, HWND};
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, CreateFontW, CreateSolidBrush, DeleteDC,
        DeleteObject, GetDC, GetTextExtentPoint32W, ReleaseDC, SelectObject, SetBkMode,
        SetTextColor, TextOutW, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CLEARTYPE_QUALITY,
        CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_PITCH, DIB_RGB_COLORS, FF_DONTCARE,
        FW_NORMAL, OUT_DEFAULT_PRECIS, TRANSPARENT,
    };
    unsafe {
        // emoji: 🔊 (U+1F50A) for unmuted, 🔇 (U+1F507) for muted
        let emoji: Vec<u16> = if muted {
            vec![0xD83D, 0xDD07] // 🔇
        } else {
            vec![0xD83D, 0xDD0A] // 🔊
        };
        let hdc_screen = GetDC(Some(HWND(std::ptr::null_mut())));
        if hdc_screen.0.is_null() {
            return None;
        }
        let hdc_mem = CreateCompatibleDC(Some(hdc_screen));
        if hdc_mem.0.is_null() {
            ReleaseDC(Some(HWND(std::ptr::null_mut())), hdc_screen);
            return None;
        }
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: 32,
                biHeight: -32, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [windows::Win32::Graphics::Gdi::RGBQUAD {
                rgbBlue: 0,
                rgbGreen: 0,
                rgbRed: 0,
                rgbReserved: 0,
            }; 1],
        };
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let hbm = match CreateDIBSection(Some(hdc_mem), &bmi, DIB_RGB_COLORS, &mut bits, None, 0) {
            Ok(h) => h,
            Err(_) => {
                let _ = DeleteDC(hdc_mem);
                ReleaseDC(Some(HWND(std::ptr::null_mut())), hdc_screen);
                return None;
            }
        };
        if bits.is_null() {
            let _ = DeleteObject(hbm.into());
            let _ = DeleteDC(hdc_mem);
            ReleaseDC(Some(HWND(std::ptr::null_mut())), hdc_screen);
            return None;
        }
        let old_bm = SelectObject(hdc_mem, hbm.into());
        let magenta = CreateSolidBrush(COLORREF(0x00FF00FF));
        let rect = windows::Win32::Foundation::RECT {
            left: 0,
            top: 0,
            right: 32,
            bottom: 32,
        };
        windows::Win32::Graphics::Gdi::FillRect(hdc_mem, &rect, magenta);
        let _ = DeleteObject(magenta.into());
        let face: Vec<u16> = "Segoe UI Emoji\0".encode_utf16().collect();
        let hfont = CreateFontW(
            -24,
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            DEFAULT_PITCH.0 as u32 | FF_DONTCARE.0 as u32,
            windows::core::PCWSTR(face.as_ptr()),
        );
        let old_font = if !hfont.0.is_null() {
            SelectObject(hdc_mem, hfont.into())
        } else {
            Default::default()
        };
        SetBkMode(hdc_mem, TRANSPARENT);
        // text color depending on theme (dark -> light, light -> dark)
        let text_color = if is_dark {
            windows::Win32::Foundation::COLORREF(0x00FFFFFF)
        } else {
            windows::Win32::Foundation::COLORREF(0x00000000)
        };
        SetTextColor(hdc_mem, text_color);
        // center emoji
        let mut sz = windows::Win32::Foundation::SIZE { cx: 0, cy: 0 };
        let _ = GetTextExtentPoint32W(hdc_mem, &emoji, &mut sz);
        let x = (32 - sz.cx) / 2;
        let y = (32 - sz.cy) / 2;
        let _ = TextOutW(hdc_mem, x, y, &emoji);
        // restore
        if !hfont.0.is_null() {
            SelectObject(hdc_mem, old_font);
            let _ = DeleteObject(hfont.into());
        }
        SelectObject(hdc_mem, old_bm);
        // convert BGRA bits to RGBA with magenta as transparent
        let pixel_count = 32 * 32;
        let src = std::slice::from_raw_parts(bits as *const u32, pixel_count);
        let mut rgba = vec![0u8; pixel_count * 4];
        for (i, &px) in src.iter().enumerate() {
            // px is 0x00RRGGBB? Actually DIB is BGR0 order: low byte B, next G, next R
            let b = (px & 0xFF) as u8;
            let g = ((px >> 8) & 0xFF) as u8;
            let r = ((px >> 16) & 0xFF) as u8;
            let is_magenta = r == 255 && g == 0 && b == 255;
            let idx = i * 4;
            if is_magenta {
                rgba[idx] = 0;
                rgba[idx + 1] = 0;
                rgba[idx + 2] = 0;
                rgba[idx + 3] = 0;
            } else {
                // if pixel is near black background that was not drawn, treat near-black as transparent? Use magenta key already.
                // For anti-aliased edges, keep semi-transparent by checking if close to magenta? Keep opaque for now.
                rgba[idx] = r;
                rgba[idx + 1] = g;
                rgba[idx + 2] = b;
                rgba[idx + 3] = 255;
                // handle aliased edges: if pixel was blended with magenta, it won't be pure magenta but will have some blend.
                // Approximate: if pixel is not pure magenta, keep opaque; simple but visible.
            }
        }
        let _ = DeleteObject(hbm.into());
        let _ = DeleteDC(hdc_mem);
        ReleaseDC(Some(HWND(std::ptr::null_mut())), hdc_screen);
        Icon::from_rgba(rgba, 32, 32).ok()
    }
}

#[cfg(windows)]
pub fn is_dark_mode() -> bool {
    use windows::Win32::System::Registry::HKEY;
    use windows::Win32::System::Registry::{
        RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ, REG_NONE, REG_VALUE_TYPE,
    };
    unsafe {
        let subkey: Vec<u16> =
            "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0"
                .encode_utf16()
                .collect();
        let mut hkey = HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            Some(0),
            KEY_READ,
            &mut hkey,
        )
        .is_err()
        {
            return false;
        }
        let valname: Vec<u16> = "AppsUseLightTheme\0".encode_utf16().collect();
        let mut data: u32 = 0;
        let mut len = std::mem::size_of::<u32>() as u32;
        let mut ty: REG_VALUE_TYPE = REG_NONE;
        let res = RegQueryValueExW(
            hkey,
            PCWSTR(valname.as_ptr()),
            None,
            Some(&mut ty),
            Some(&mut data as *mut u32 as *mut u8),
            Some(&mut len),
        );
        let _ = windows::Win32::System::Registry::RegCloseKey(hkey);
        if res.is_ok() {
            return data == 0;
        }
        false
    }
}

#[cfg(not(windows))]
pub fn is_dark_mode() -> bool {
    false
}

// ------------------------------------------------------------
// logging
// ------------------------------------------------------------
pub fn log_verbose(cfg: &AppConfig, msg: &str) {
    if !cfg.verbose_log {
        return;
    }
    let path = std::env::var("TEMP")
        .map(|t| std::path::PathBuf::from(t).join("audio-switcher-rust.log"))
        .unwrap_or_else(|_| std::path::PathBuf::from("audio-switcher-rust.log"));
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "[{:?}] {}", std::time::SystemTime::now(), msg);
    }
}

// ------------------------------------------------------------
// menu building
// ------------------------------------------------------------
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

    let mut device_menu_items: Vec<CheckMenuItem> = Vec::new();
    for dev in devices {
        let checked = default_id == Some(dev.id.as_str());
        let item = CheckMenuItem::with_id(
            format!("device_{}", dev.id),
            dev.name.clone(),
            true,
            checked,
            None,
        );
        device_menu_items.push(item);
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
    let vol_custom = MenuItem::with_id(
        "vol_custom",
        tr("custom", lang),
        cfg.volume_limit_enabled,
        None,
    );
    let vol_sub = Submenu::with_id_and_items(
        "volume_limit",
        tr("volume_limit", lang),
        true,
        &[
            &vol_enabled,
            &PredefinedMenuItem::separator(),
            &vol_25,
            &vol_50,
            &vol_custom,
        ],
    )
    .unwrap();

    let wheel = CheckMenuItem::with_id(
        "wheel_accel",
        tr("wheel_accel", lang),
        true,
        cfg.wheel_acceleration,
        None,
    );
    let verbose = CheckMenuItem::with_id(
        "verbose_log",
        tr("verbose_log", lang),
        true,
        cfg.verbose_log,
        None,
    );
    let exp_sub = Submenu::with_id_and_items(
        "experimental",
        tr("experimental", lang),
        true,
        &[&wheel, &verbose],
    )
    .unwrap();

    let open_mixer = MenuItem::with_id("open_mixer", tr("open_mixer", lang), true, None);
    let open_sound = MenuItem::with_id("open_sound", tr("open_sound", lang), true, None);

    let autostart = CheckMenuItem::with_id(
        "autostart",
        tr("autostart", lang),
        true,
        cfg.autostart,
        None,
    );

    let lang_zh =
        CheckMenuItem::with_id("lang_zh", tr("chinese", lang), true, cfg.lang == "zh", None);
    let lang_en =
        CheckMenuItem::with_id("lang_en", tr("english", lang), true, cfg.lang == "en", None);
    let lang_sub =
        Submenu::with_id_and_items("language", "Language", true, &[&lang_zh, &lang_en]).unwrap();

    let about = MenuItem::with_id("about", tr("about", lang), true, None);
    let exit = MenuItem::with_id("exit", tr("exit", lang), true, None);

    let menu = Menu::new();
    for item in &device_menu_items {
        let _ = menu.append(item);
    }
    if !device_menu_items.is_empty() {
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

// ------------------------------------------------------------
// tray wrapper
// ------------------------------------------------------------
pub struct TrayWrapper {
    pub tray: TrayIcon,
    pub handles: MenuHandles,
}

impl TrayWrapper {
    pub fn new(
        cfg: &AppConfig,
        devices: &[AudioDevice],
        default_id: Option<&str>,
        muted: bool,
    ) -> Self {
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

// ------------------------------------------------------------
// helpers for system entries
// ------------------------------------------------------------
#[cfg(windows)]
pub fn open_volume_mixer() {
    unsafe {
        let op: Vec<u16> = "open\0".encode_utf16().collect();
        let file: Vec<u16> = "SndVol.exe\0".encode_utf16().collect();
        let res = windows::Win32::UI::Shell::ShellExecuteW(
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
            MessageBoxW(
                None,
                PCWSTR(msg.as_ptr()),
                PCWSTR(title.as_ptr()),
                MB_OK | MB_ICONWARNING,
            );
        }
    }
}

#[cfg(not(windows))]
pub fn open_volume_mixer() {}

#[cfg(windows)]
pub fn open_sound_settings() {
    unsafe {
        let op: Vec<u16> = "open\0".encode_utf16().collect();
        let file: Vec<u16> = "control\0".encode_utf16().collect();
        let params: Vec<u16> = "mmsys.cpl\0".encode_utf16().collect();
        let res = windows::Win32::UI::Shell::ShellExecuteW(
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
            MessageBoxW(
                None,
                PCWSTR(msg.as_ptr()),
                PCWSTR(title.as_ptr()),
                MB_OK | MB_ICONWARNING,
            );
        }
    }
}

#[cfg(not(windows))]
pub fn open_sound_settings() {}

#[cfg(windows)]
pub fn show_about(lang: &str) {
    let _ = lang;
    unsafe {
        use windows::core::{w, PCWSTR};
        use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
        use windows::Win32::Graphics::Gdi::HBRUSH;
        use windows::Win32::System::LibraryLoader::GetModuleHandleW;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
            IsWindow, LoadCursorW, PostQuitMessage, RegisterClassW, TranslateMessage,
            IDC_ARROW, MSG, WINDOW_EX_STYLE, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_DESTROY,
            WNDCLASSW, WS_CAPTION, WS_OVERLAPPED, WS_SYSMENU, WS_VISIBLE,
        };
        unsafe extern "system" fn wndproc_about(
            hwnd: HWND,
            msg: u32,
            wparam: WPARAM,
            lparam: LPARAM,
        ) -> LRESULT {
            use windows::core::w;
            
            use windows::Win32::UI::WindowsAndMessaging::{
                CreateWindowExW as CreateW, HMENU, WS_CHILD, WS_VISIBLE,
            };
            match msg {
                WM_CREATE => {
                    let hinst = GetModuleHandleW(PCWSTR::null()).unwrap();
                    let hinst2 =
                        windows::Win32::Foundation::HINSTANCE(hinst.0);
                    let _ = CreateW(
                        WINDOW_EX_STYLE(0),
                        w!("STATIC"),
                        w!("GitHub:"),
                        WS_CHILD | WS_VISIBLE,
                        15,
                        20,
                        60,
                        20,
                        Some(hwnd),
                        None,
                        Some(hinst2),
                        None,
                    );
                    let _ = CreateW(
                        WINDOW_EX_STYLE(0),
                        w!("BUTTON"),
                        w!("https://github.com"),
                        WS_CHILD | WS_VISIBLE,
                        80,
                        20,
                        260,
                        20,
                        Some(hwnd),
                        Some(HMENU(101 as *mut std::ffi::c_void)),
                        Some(hinst2),
                        None,
                    );
                    let _ = CreateW(
                        WINDOW_EX_STYLE(0),
                        w!("BUTTON"),
                        w!("确定"),
                        WS_CHILD | WS_VISIBLE,
                        150,
                        80,
                        80,
                        26,
                        Some(hwnd),
                        Some(HMENU(std::ptr::dangling_mut::<std::ffi::c_void>())),
                        Some(hinst2),
                        None,
                    );
                    LRESULT(0)
                }
                WM_COMMAND => {
                    let id = (wparam.0 & 0xFFFF) as u16;
                    if id == 1 {
                        let _ = DestroyWindow(hwnd);
                    } else if id == 101 {
                        let url: Vec<u16> = "https://github.com\0".encode_utf16().collect();
                        let op: Vec<u16> = "open\0".encode_utf16().collect();
                        let _ = ShellExecuteW(
                            None,
                            PCWSTR(op.as_ptr()),
                            PCWSTR(url.as_ptr()),
                            PCWSTR::null(),
                            PCWSTR::null(),
                            windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
                        );
                    }
                    LRESULT(0)
                }
                WM_CLOSE => {
                    let _ = DestroyWindow(hwnd);
                    LRESULT(0)
                }
                WM_DESTROY => {
                    PostQuitMessage(0);
                    LRESULT(0)
                }
                _ => DefWindowProcW(hwnd, msg, wparam, lparam),
            }
        }
        let hinst = GetModuleHandleW(PCWSTR::null()).unwrap();
        let hinst2 = windows::Win32::Foundation::HINSTANCE(hinst.0);
        let class_name = w!("AudioSwitcherAbout");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc_about),
            hInstance: hinst2,
            lpszClassName: class_name,
            hbrBackground: HBRUSH(16 as *mut std::ffi::c_void),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&wc);
        let hwnd = match CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            w!("关于"),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            100,
            100,
            380,
            160,
            None,
            None,
            Some(hinst2),
            None,
        ) {
            Ok(h) => h,
            Err(_) => return,
        };
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
            if !IsWindow(Some(hwnd)).as_bool() {
                break;
            }
        }
    }
}

#[cfg(not(windows))]
pub fn show_about(_lang: &str) {}

#[cfg(windows)]
pub fn show_error_invalid_custom(lang: &str) {
    unsafe {
        let txt = tr("invalid_custom", lang);
        let wide: Vec<u16> = txt.encode_utf16().chain(std::iter::once(0)).collect();
        let title: Vec<u16> = "Audio Switcher\0".encode_utf16().collect();
        MessageBoxW(
            None,
            PCWSTR(wide.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONWARNING,
        );
    }
}

#[cfg(not(windows))]
pub fn show_error_invalid_custom(_lang: &str) {}

#[cfg(windows)]
pub fn prompt_custom_limit(lang: &str) -> Option<u32> {
    use parking_lot::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::Graphics::Gdi::HBRUSH;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
        GetWindowTextW, IsWindow, LoadCursorW, PostQuitMessage, RegisterClassW, TranslateMessage,
        IDC_ARROW, MSG, WINDOW_EX_STYLE, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_DESTROY, WNDCLASSW,
        WS_CAPTION, WS_EX_CLIENTEDGE, WS_OVERLAPPED, WS_SYSMENU, WS_VISIBLE,
    };

    static RESULT: Mutex<Option<Option<u32>>> = Mutex::new(None);
    static DONE: AtomicBool = AtomicBool::new(false);
    static IS_ZH: AtomicBool = AtomicBool::new(true);
    static EDIT_HWND: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    unsafe extern "system" fn wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        use windows::core::w;
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW as CreateW, HMENU, WS_CHILD, WS_VISIBLE,
        };
        match msg {
            WM_CREATE => {
                let hinst = GetModuleHandleW(PCWSTR::null()).unwrap();
                let hinst2 = windows::Win32::Foundation::HINSTANCE(hinst.0);
                let is_zh = IS_ZH.load(Ordering::SeqCst);
                let label_text = if is_zh {
                    w!("输入 1-100 整数:")
                } else {
                    w!("Enter 1-100:")
                };
                let _ = CreateW(
                    WINDOW_EX_STYLE(0),
                    w!("STATIC"),
                    label_text,
                    WS_CHILD | WS_VISIBLE,
                    10,
                    10,
                    300,
                    20,
                    Some(hwnd),
                    None,
                    Some(hinst2),
                    None,
                );
                let edit = CreateW(
                    WS_EX_CLIENTEDGE,
                    w!("EDIT"),
                    w!(""),
                    WS_CHILD | WS_VISIBLE | windows::Win32::UI::WindowsAndMessaging::WS_BORDER,
                    10,
                    30,
                    300,
                    24,
                    Some(hwnd),
                    None,
                    Some(hinst2),
                    None,
                )
                .unwrap_or(HWND(std::ptr::null_mut()));
                EDIT_HWND.store(edit.0 as usize, Ordering::SeqCst);
                let _ = SetFocus(Some(edit));
                let ok_text = if is_zh { w!("确定") } else { w!("OK") };
                let cancel_text = if is_zh { w!("取消") } else { w!("Cancel") };
                let _ = CreateW(
                    WINDOW_EX_STYLE(0),
                    w!("BUTTON"),
                    ok_text,
                    WS_CHILD | WS_VISIBLE,
                    80,
                    70,
                    70,
                    24,
                    Some(hwnd),
                    Some(HMENU(std::ptr::dangling_mut::<std::ffi::c_void>())),
                    Some(hinst2),
                    None,
                );
                let _ = CreateW(
                    WINDOW_EX_STYLE(0),
                    w!("BUTTON"),
                    cancel_text,
                    WS_CHILD | WS_VISIBLE,
                    170,
                    70,
                    70,
                    24,
                    Some(hwnd),
                    Some(HMENU(2 as *mut std::ffi::c_void)),
                    Some(hinst2),
                    None,
                );
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = (wparam.0 & 0xFFFF) as u16;
                if id == 1 {
                    let edit_hwnd = HWND(EDIT_HWND.load(Ordering::SeqCst) as *mut std::ffi::c_void);
                    let mut buf = [0u16; 64];
                    let len = GetWindowTextW(edit_hwnd, &mut buf);
                    let s = String::from_utf16_lossy(&buf[..len as usize]);
                    match AppConfig::validate_custom_limit(&s) {
                        Ok(v) => {
                            *RESULT.lock() = Some(Some(v));
                            DONE.store(true, Ordering::SeqCst);
                            let _ = DestroyWindow(hwnd);
                        }
                        Err(_) => {
                            let is_zh = IS_ZH.load(Ordering::SeqCst);
                            show_error_invalid_custom(if is_zh { "zh" } else { "en" });
                        }
                    }
                } else if id == 2 {
                    let mut lock = RESULT.lock();
                    *lock = Some(None);
                    DONE.store(true, Ordering::SeqCst);
                    let _ = DestroyWindow(hwnd);
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                let mut lock = RESULT.lock();
                if lock.is_none() {
                    *lock = Some(None);
                }
                DONE.store(true, Ordering::SeqCst);
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    unsafe {
        *RESULT.lock() = None;
        DONE.store(false, Ordering::SeqCst);
        IS_ZH.store(lang == "zh", Ordering::SeqCst);
        EDIT_HWND.store(0, Ordering::SeqCst);
        let hinst = GetModuleHandleW(PCWSTR::null()).unwrap();
        let hinst2 = windows::Win32::Foundation::HINSTANCE(hinst.0);
        let class_name = w!("AudioSwitcherPrompt");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinst2,
            lpszClassName: class_name,
            hbrBackground: HBRUSH(16 as *mut std::ffi::c_void),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&wc);
        let title = if lang == "zh" {
            w!("自定义音量上限")
        } else {
            w!("Custom Volume Limit")
        };
        let hwnd = match CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            title,
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            100,
            100,
            340,
            150,
            None,
            None,
            Some(hinst2),
            None,
        ) {
            Ok(h) => h,
            Err(_) => return None,
        };
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
            if DONE.load(Ordering::SeqCst) {
                if IsWindow(Some(hwnd)).as_bool() {
                    let _ = DestroyWindow(hwnd);
                }
                break;
            }
            if !IsWindow(Some(hwnd)).as_bool() && !DONE.load(Ordering::SeqCst) {
                DONE.store(true, Ordering::SeqCst);
                break;
            }
        }
        let lock = *RESULT.lock();
        match lock {
            Some(Some(v)) if (1..=100).contains(&v) => Some(v),
            Some(Some(_)) => {
                show_error_invalid_custom(lang);
                None
            }
            _ => None,
        }
    }
}
#[cfg(not(windows))]
pub fn prompt_custom_limit(_lang: &str) -> Option<u32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn i18n_zh_en() {
        assert_eq!(tr("mute", "zh"), "全局静音");
        assert_eq!(tr("mute", "en"), "Mute");
        assert_eq!(tr("volume_limit", "zh"), "音量上限");
        assert_eq!(tr("experimental", "en"), "Experimental");
    }

    #[test]
    fn tooltip_format() {
        let dev = AudioDevice {
            id: "a".into(),
            name: "Realtek Speaker".into(),
        };
        assert_eq!(
            format_tooltip(Some(&dev), 62, false, "zh"),
            "Realtek Speaker - 62%"
        );
        assert_eq!(format_tooltip(Some(&dev), 0, true, "zh"), "静音");
        assert_eq!(format_tooltip(Some(&dev), 0, true, "en"), "Muted");
        let long = AudioDevice {
            id: "a".into(),
            name: "A".repeat(100),
        };
        let tip = format_tooltip(Some(&long), 50, false, "zh");
        assert!(tip.chars().count() <= 60);
    }

    #[test]
    fn wheel_calc() {
        assert_eq!(calc_wheel_step(1, 200, true), 1);
        assert_eq!(calc_wheel_step(3, 100, true), 2);
        assert_eq!(calc_wheel_step(5, 100, true), 5);
        assert_eq!(calc_wheel_step(3, 50, true), 5);
        assert_eq!(calc_wheel_step(5, 50, false), 1);
    }

    #[test]
    fn wheel_state_progression() {
        let mut ws = WheelState::new();
        let base = Instant::now();
        let s1 = ws.push(base, true, 120);
        assert_eq!(s1, 1);
        let s2 = ws.push(base + Duration::from_millis(50), true, 120);
        assert_eq!(s2, 5); // <80ms => 5%
        let s3 = ws.push(base + Duration::from_millis(100), true, 120);
        assert_eq!(s3, 5);
        let mut ws2 = WheelState::new();
        let b = Instant::now();
        assert_eq!(ws2.push(b, true, 120), 1);
        assert_eq!(ws2.push(b + Duration::from_millis(90), true, 120), 1); // 90ms >80, count2 =>1
        assert_eq!(ws2.push(b + Duration::from_millis(180), true, 120), 2); // count3 =>2
        assert_eq!(ws2.push(b + Duration::from_millis(270), true, 120), 2); // still within 200ms window? first at 0 expires? count ~3 =>2
                                                                            // test delta/120 stacking: 240 => 2 ticks
        let mut ws3 = WheelState::new();
        let s = ws3.push(b, true, 240);
        // Actually 240 at once gives 2 ticks with 0 interval => 5 per spec cap
        assert_eq!(s, 5);
    }
    #[test]
    fn clamp_via_config() {
        let mut cfg = AppConfig {
            volume_limit_enabled: true,
            volume_limit: 30,
            ..Default::default()
        };
        use crate::config::clamp_volume;
        assert_eq!(clamp_volume(80, &cfg), 30);
        cfg.volume_limit_enabled = false;
        assert_eq!(clamp_volume(80, &cfg), 80);
    }
}

//! Windows dialogs: custom limit prompt + error boxes.

#[cfg(windows)]
use windows::core::PCWSTR;
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONWARNING, MB_OK};

use crate::ui::i18n::tr;

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
pub fn show_autostart_error() {
    unsafe {
        let msg: Vec<u16> = "设置开机自启失败\0".encode_utf16().collect();
        let title: Vec<u16> = "Audio Switcher\0".encode_utf16().collect();
        MessageBoxW(
            None,
            PCWSTR(msg.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONWARNING,
        );
    }
}

#[cfg(not(windows))]
pub fn show_autostart_error() {}

#[cfg(windows)]
#[allow(dead_code)]
pub fn show_msgbox(msg: &str) {
    unsafe {
        let wide: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
        let title: Vec<u16> = "Audio Switcher\0".encode_utf16().collect();
        MessageBoxW(
            None,
            PCWSTR(wide.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONWARNING,
        );
    }
}

#[cfg(windows)]
pub fn prompt_custom_limit(lang: &str) -> Option<u32> {
    use parking_lot::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};
    use windows::core::w;
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
                let label_text = if is_zh { w!("输入 1-100 整数:") } else { w!("Enter 1-100:") };
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
                    match crate::config::AppConfig::validate_custom_limit(&s) {
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
                    *RESULT.lock() = Some(None);
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
        let title = if lang == "zh" { w!("自定义音量上限") } else { w!("Custom Volume Limit") };
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
        match *RESULT.lock() {
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

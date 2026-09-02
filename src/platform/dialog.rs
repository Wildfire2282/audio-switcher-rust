//! Windows dialogs: custom limit prompt + error boxes.

#[cfg(windows)]
use windows::core::PCWSTR;

use crate::config::Lang;
use crate::ui::i18n::tr;

// ---------------------------------------------------------------------------
// Centered MessageBox helper (hook-based)
// ---------------------------------------------------------------------------
#[cfg(windows)]
fn show_centered_message_box_raw(text: PCWSTR, title: PCWSTR, style: windows::Win32::UI::WindowsAndMessaging::MESSAGEBOX_STYLE) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::{
        HHOOK, SetWindowsHookExW, WH_CBT,
    };

    static HOOK: AtomicUsize = AtomicUsize::new(0);

    unsafe extern "system" fn cbt_hook(n_code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        use std::sync::atomic::Ordering;
        use windows::Win32::Foundation::{HWND, RECT};
        use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
        use windows::Win32::UI::WindowsAndMessaging::{CallNextHookEx, GetWindowRect, HHOOK, HCBT_ACTIVATE, SetWindowPos, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER};

        if n_code == HCBT_ACTIVATE as i32 {
            let hwnd = HWND(wparam.0 as *mut std::ffi::c_void);
            // SAFETY: hwnd is the MessageBox window being activated.
            let mut rect = RECT::default();
            if unsafe { GetWindowRect(hwnd, &mut rect).is_ok() } {
                let w = rect.right - rect.left;
                let h = rect.bottom - rect.top;
                // Try monitor work area first (excludes taskbar), fallback to full screen.
                let (sw, sh, off_x, off_y) = unsafe {
                    let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
                    let mut mi = MONITORINFO {
                        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                        ..Default::default()
                    };
                    if GetMonitorInfoW(monitor, &mut mi).as_bool() {
                        let rw = mi.rcWork.right - mi.rcWork.left;
                        let rh = mi.rcWork.bottom - mi.rcWork.top;
                        (rw, rh, mi.rcWork.left, mi.rcWork.top)
                    } else {
                        use windows::Win32::UI::WindowsAndMessaging::{SM_CXSCREEN, SM_CYSCREEN, GetSystemMetrics};
                        (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN), 0, 0)
                    }
                };
                let x = off_x + (sw - w) / 2;
                let y = off_y + (sh - h) / 2;
                // SAFETY: SetWindowPos with valid hwnd and SWP_NOSIZE.
                let _ = unsafe { SetWindowPos(hwnd, None, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE) };
            }
        }
        // SAFETY: CallNextHookEx with stored hook handle.
        let hook_val = HOOK.load(Ordering::SeqCst);
        let hook_opt = if hook_val == 0 { None } else { Some(HHOOK(hook_val as *mut std::ffi::c_void)) };
        unsafe { CallNextHookEx(hook_opt, n_code, wparam, lparam) }
    }

    // SAFETY: SetWindowsHookExW with current thread id.
    let hook = unsafe {
        let tid = GetCurrentThreadId();
        SetWindowsHookExW(WH_CBT, Some(cbt_hook), None, tid).unwrap_or(HHOOK(std::ptr::null_mut()))
    };
    HOOK.store(hook.0 as usize, Ordering::SeqCst);

    // SAFETY: MessageBoxW with null-terminated PCWSTRs kept alive by caller.
    unsafe {
        windows::Win32::UI::WindowsAndMessaging::MessageBoxW(None, text, title, style);
    }

    // Unhook if still installed (hook proc may have already unhooked on some paths).
    let cur = HOOK.load(Ordering::SeqCst);
    if cur != 0 {
        // SAFETY: UnhookWindowsHookEx with valid HHOOK.
        let _ = unsafe { windows::Win32::UI::WindowsAndMessaging::UnhookWindowsHookEx(HHOOK(cur as *mut std::ffi::c_void)) };
        HOOK.store(0, Ordering::SeqCst);
    }
    // Ensure any remaining hook is cleared.
    if hook.0 as usize != 0 && HOOK.load(Ordering::SeqCst) == hook.0 as usize {
        HOOK.store(0, Ordering::SeqCst);
    }
}

#[cfg(windows)]
fn centered_msgbox(text_wide: &[u16], title_wide: &[u16]) {
    use windows::Win32::UI::WindowsAndMessaging::{MB_ICONWARNING, MB_OK};
    show_centered_message_box_raw(
        PCWSTR(text_wide.as_ptr()),
        PCWSTR(title_wide.as_ptr()),
        MB_OK | MB_ICONWARNING,
    );
}

#[cfg(windows)]
pub(crate) fn show_error_invalid_custom(lang: Lang) {
    let txt = tr("invalid_custom", lang);
    let wide: Vec<u16> = txt.encode_utf16().chain(std::iter::once(0)).collect();
    let title: Vec<u16> = "Audio Switcher\0".encode_utf16().collect();
    centered_msgbox(&wide, &title);
}

#[cfg(not(windows))]
pub(crate) fn show_error_invalid_custom(_lang: Lang) {}

#[cfg(windows)]
pub(crate) fn show_autostart_error() {
    let msg: Vec<u16> = "设置开机自启失败\0".encode_utf16().collect();
    let title: Vec<u16> = "Audio Switcher\0".encode_utf16().collect();
    centered_msgbox(&msg, &title);
}

#[cfg(not(windows))]
pub(crate) fn show_autostart_error() {}

#[cfg(windows)]
pub(crate) fn show_msgbox(msg: &str) {
    let wide: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
    let title: Vec<u16> = "Audio Switcher\0".encode_utf16().collect();
    centered_msgbox(&wide, &title);
}

#[cfg(not(windows))]
pub(crate) fn show_msgbox(_msg: &str) {}

#[cfg(windows)]
pub(crate) fn prompt_custom_limit(lang: Lang) -> Option<u32> {
    use parking_lot::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};
    use windows::core::w;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::Graphics::Gdi::HBRUSH;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
        GetSystemMetrics, GetWindowTextW, IsWindow, LoadCursorW, PostQuitMessage, RegisterClassW,
        TranslateMessage, IDC_ARROW, MSG, SM_CXSCREEN, SM_CYSCREEN, WINDOW_EX_STYLE, WM_CLOSE,
        WM_COMMAND, WM_CREATE, WM_DESTROY, WNDCLASSW, WS_CAPTION, WS_EX_CLIENTEDGE, WS_OVERLAPPED,
        WS_SYSMENU, WS_VISIBLE,
    };

    const ID_OK: u16 = 1;
    const ID_CANCEL: u16 = 2;
    const BN_CLICKED: u16 = 0;

    const WIN_W: i32 = 340;
    const WIN_H: i32 = 150;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum DialogOutcome {
        Pending,
        Cancel,
        Value(u32),
    }

    static RESULT: Mutex<DialogOutcome> = Mutex::new(DialogOutcome::Pending);
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
                // SAFETY: GetModuleHandleW(null) returns current module handle.
                let hinst = unsafe { GetModuleHandleW(PCWSTR::null()).unwrap() };
                let hinst2 = windows::Win32::Foundation::HINSTANCE(hinst.0);
                let is_zh = IS_ZH.load(Ordering::SeqCst);
                let label_text =
                    if is_zh { w!("输入 1-100 整数:") } else { w!("Enter 1-100:") };
                // SAFETY: static strings, valid hinst2, hwnd valid.
                unsafe {
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
                }
                // SAFETY: EDIT creation with valid params.
                unsafe {
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
                    let edit_hwnd = HWND(EDIT_HWND.load(Ordering::SeqCst) as *mut std::ffi::c_void);
                    let _ = SetFocus(Some(edit_hwnd));
                }
                let ok_text = if is_zh { w!("确定") } else { w!("OK") };
                let cancel_text = if is_zh { w!("取消") } else { w!("Cancel") };
                // SAFETY: BUTTON creation with integer HMENU IDs.
                unsafe {
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
                        Some(HMENU(ID_OK as *mut std::ffi::c_void)),
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
                        Some(HMENU(ID_CANCEL as *mut std::ffi::c_void)),
                        Some(hinst2),
                        None,
                    );
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                let code = ((wparam.0 >> 16) & 0xFFFF) as u16;
                let id = (wparam.0 & 0xFFFF) as u16;
                if code == BN_CLICKED && id == ID_OK {
                    let edit_hwnd = HWND(EDIT_HWND.load(Ordering::SeqCst) as *mut std::ffi::c_void);
                    let mut buf = [0u16; 64];
                    // SAFETY: edit_hwnd is EDIT control created above; buf is 64 wide chars.
                    let len = unsafe { GetWindowTextW(edit_hwnd, &mut buf) };
                    let s = String::from_utf16_lossy(&buf[..len as usize]);
                    match crate::config::AppConfig::validate_custom_limit(&s) {
                        Ok(v) => {
                            *RESULT.lock() = DialogOutcome::Value(v);
                            DONE.store(true, Ordering::SeqCst);
                            // SAFETY: hwnd valid.
                            unsafe {
                                let _ = DestroyWindow(hwnd);
                            }
                        }
                        Err(_) => {
                            let is_zh = IS_ZH.load(Ordering::SeqCst);
                            show_error_invalid_custom(if is_zh { Lang::Zh } else { Lang::En });
                        }
                    }
                } else if code == BN_CLICKED && id == ID_CANCEL {
                    *RESULT.lock() = DialogOutcome::Cancel;
                    DONE.store(true, Ordering::SeqCst);
                    // SAFETY: hwnd valid.
                    unsafe {
                        let _ = DestroyWindow(hwnd);
                    }
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                let mut lock = RESULT.lock();
                if *lock == DialogOutcome::Pending {
                    *lock = DialogOutcome::Cancel;
                }
                DONE.store(true, Ordering::SeqCst);
                // SAFETY: hwnd valid.
                unsafe {
                    let _ = DestroyWindow(hwnd);
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                // SAFETY: PostQuitMessage always safe.
                unsafe {
                    PostQuitMessage(0);
                }
                LRESULT(0)
            }
            _ => {
                // SAFETY: DefWindowProcW for unhandled messages.
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
        }
    }

    *RESULT.lock() = DialogOutcome::Pending;
    DONE.store(false, Ordering::SeqCst);
    IS_ZH.store(lang == Lang::Zh, Ordering::SeqCst);
    EDIT_HWND.store(0, Ordering::SeqCst);
    // SAFETY: GetModuleHandleW(null) returns current module handle.
    let hinst = unsafe { GetModuleHandleW(PCWSTR::null()).unwrap() };
    let hinst2 = windows::Win32::Foundation::HINSTANCE(hinst.0);
    let class_name = w!("AudioSwitcherPrompt");
    static CLASS_ONCE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    CLASS_ONCE.get_or_init(|| {
        // SAFETY: WNDCLASSW valid, strings static, wndproc valid.
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinst2,
            lpszClassName: class_name,
            // COLOR_WINDOW (5) + 1 as HBRUSH per Win32 convention
            hbrBackground: HBRUSH((5 + 1) as *mut std::ffi::c_void),
            // SAFETY: LoadCursorW with IDC_ARROW is always valid.
            hCursor: unsafe { LoadCursorW(None, IDC_ARROW).unwrap_or_default() },
            ..Default::default()
        };
        // SAFETY: RegisterClassW with valid WNDCLASSW.
        unsafe {
            RegisterClassW(&wc);
        }
        true
    });
    let title =
        if lang == Lang::Zh { w!("自定义音量上限") } else { w!("Custom Volume Limit") };
    // Center on screen (work area aware): compute centered x/y from screen metrics / monitor work area.
    let (cx, cy) = unsafe {
        // Try monitor work area via primary monitor's metrics as fallback.
        // Use GetSystemMetrics for simplicity; hook for MessageBox uses monitor work area.
        let sw = GetSystemMetrics(SM_CXSCREEN);
        let sh = GetSystemMetrics(SM_CYSCREEN);
        // If metrics return 0 (unlikely), fall back to 100,100.
        if sw > WIN_W && sh > WIN_H {
            ((sw - WIN_W) / 2, (sh - WIN_H) / 2)
        } else {
            (100, 100)
        }
    };
    // SAFETY: CreateWindowExW with valid params.
    let hwnd = match unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            title,
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            cx,
            cy,
            WIN_W,
            WIN_H,
            None,
            None,
            Some(hinst2),
            None,
        )
    } {
        Ok(h) => h,
        Err(_) => return None,
    };
    let mut msg = MSG::default();
    // SAFETY: Message loop FFI valid.
    unsafe {
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
    }
    match *RESULT.lock() {
        DialogOutcome::Value(v) if (1..=100).contains(&v) => Some(v),
        DialogOutcome::Value(_) => {
            show_error_invalid_custom(lang);
            None
        }
        DialogOutcome::Cancel | DialogOutcome::Pending => None,
    }
}

#[cfg(not(windows))]
pub(crate) fn prompt_custom_limit(_lang: Lang) -> Option<u32> {
    None
}

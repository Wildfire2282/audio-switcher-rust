//! Windows dialogs: error boxes (centered).

#[cfg(windows)]
use windows::core::PCWSTR;

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


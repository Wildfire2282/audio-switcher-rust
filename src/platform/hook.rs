//! Windows low-level mouse wheel hook, encapsulated.

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};

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
    use windows::Win32::UI::WindowsAndMessaging::{CallNextHookEx, MSLLHOOKSTRUCT, WM_MOUSEWHEEL};
    if n_code >= 0 && w_param.0 as u32 == WM_MOUSEWHEEL {
        let info = &*(l_param.0 as *const MSLLHOOKSTRUCT);
        let delta = (info.mouseData >> 16) as u16 as i16 as i32;
        WHEEL_DELTA.fetch_add(delta, Ordering::Relaxed);
        WHEEL_PENDING.store(true, Ordering::Relaxed);
    }
    CallNextHookEx(None, n_code, w_param, l_param)
}

/// RAII hook handle. Drop uninstalls.
pub struct WheelHook {
    _private: (),
}

impl WheelHook {
    /// Try to install hook on current thread (needs message pump).
    #[allow(dead_code)]
    pub fn try_install_if_needed() {
        #[cfg(windows)]
        {
            if HOOK_HANDLE.load(Ordering::Relaxed) == 0 {
                let _ = Self::install();
            }
        }
    }
    pub fn install() -> Option<Self> {
        #[cfg(windows)]
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{SetWindowsHookExW, WH_MOUSE_LL};
            if HOOK_HANDLE.load(Ordering::Relaxed) != 0 {
                return Some(Self { _private: () });
            }
            if let Ok(hook) = SetWindowsHookExW(WH_MOUSE_LL, Some(hook_proc), None, 0) {
                HOOK_HANDLE.store(hook.0 as usize, Ordering::Relaxed);
                return Some(Self { _private: () });
            }
            None
        }
        #[cfg(not(windows))]
        {
            Some(Self { _private: () })
        }
    }

}

impl Drop for WheelHook {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            let raw = HOOK_HANDLE.swap(0, Ordering::Relaxed);
            if raw != 0 {
                unsafe {
                    let hook =
                        windows::Win32::UI::WindowsAndMessaging::HHOOK(raw as *mut std::ffi::c_void);
                    let _ = windows::Win32::UI::WindowsAndMessaging::UnhookWindowsHookEx(hook);
                }
            }
        }
    }
}

// ---- stateless helpers for App ----

pub fn take_pending() -> bool {
    WHEEL_PENDING.swap(false, Ordering::Relaxed)
}

pub fn peek_pending() -> bool {
    WHEEL_PENDING.load(Ordering::Relaxed)
}

pub fn take_delta() -> i32 {
    WHEEL_DELTA.swap(0, Ordering::Relaxed)
}

#[cfg(windows)]
pub fn cursor_over_tray(wrapper: &crate::ui::tray::TrayWrapper) -> bool {
    unsafe {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
        let mut pt = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut pt).is_err() {
            return false;
        }
        if let Some(rect) = wrapper.tray.rect() {
            let x = pt.x as f64;
            let y = pt.y as f64;
            let pad = 16.0;
            x >= rect.position.x - pad
                && x < rect.position.x + rect.size.width as f64 + pad
                && y >= rect.position.y - pad
                && y < rect.position.y + rect.size.height as f64 + pad
        } else {
            true
        }
    }
}

#[cfg(not(windows))]
pub fn cursor_over_tray(_wrapper: &crate::ui::tray::TrayWrapper) -> bool {
    false
}

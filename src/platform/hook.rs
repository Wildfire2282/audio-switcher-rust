//! Windows low-level mouse wheel hook, encapsulated.
//!
//! The hook is installed lazily via [`WheelHook::install`] and automatically
//! removed on drop. Global atomics communicate wheel events to the main loop.

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};

/// Accumulated wheel delta (WHEEL_DELTA is signed 120-per-notch).
static WHEEL_DELTA: AtomicI32 = AtomicI32::new(0);
/// Whether a wheel event is pending consumption.
static WHEEL_PENDING: AtomicBool = AtomicBool::new(false);
#[cfg(windows)]
static HOOK_HANDLE: AtomicUsize = AtomicUsize::new(0);

#[cfg(windows)]
// SAFETY: called by Windows on the hook thread; l_param is valid MSLLHOOKSTRUCT when n_code >=0.
unsafe extern "system" fn hook_proc(
    n_code: i32,
    w_param: windows::Win32::Foundation::WPARAM,
    l_param: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{CallNextHookEx, MSLLHOOKSTRUCT, WM_MOUSEWHEEL};
    if n_code >= 0 && w_param.0 as u32 == WM_MOUSEWHEEL {
        // SAFETY: per Win32 contract l_param points to MSLLHOOKSTRUCT
        let info = unsafe { &*(l_param.0 as *const MSLLHOOKSTRUCT) };
        let delta = (info.mouseData >> 16) as u16 as i16 as i32;
        // SeqCst to ensure visibility even if OS dispatches on a different thread.
        WHEEL_DELTA.fetch_add(delta, Ordering::SeqCst);
        WHEEL_PENDING.store(true, Ordering::SeqCst);
    }
    // SAFETY: CallNextHookEx is always safe to forward.
    unsafe { CallNextHookEx(None, n_code, w_param, l_param) }
}

/// RAII hook handle. Drop uninstalls the hook exactly once.
pub struct WheelHook {
    _private: (),
}

impl WheelHook {
    /// Install the low-level mouse hook.
    ///
    /// Returns `None` on Windows if `SetWindowsHookExW` fails.
    #[must_use]
    pub fn install() -> Option<Self> {
        #[cfg(windows)]
        {
            // Check if already installed — Relaxed is sufficient for the handle guard.
            if HOOK_HANDLE.load(Ordering::Relaxed) != 0 {
                return Some(Self { _private: () });
            }
            // SAFETY: WH_MOUSE_LL is process-global, hook_proc has correct signature.
            let hook = unsafe {
                use windows::Win32::UI::WindowsAndMessaging::{SetWindowsHookExW, WH_MOUSE_LL};
                SetWindowsHookExW(WH_MOUSE_LL, Some(hook_proc), None, 0).ok()
            };
            if let Some(hook) = hook {
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
                // SAFETY: raw came from SetWindowsHookExW; balances exactly once.
                unsafe {
                    let hook = windows::Win32::UI::WindowsAndMessaging::HHOOK(
                        raw as *mut std::ffi::c_void,
                    );
                    let _ = windows::Win32::UI::WindowsAndMessaging::UnhookWindowsHookEx(hook);
                }
            }
        }
    }
}

// ---- stateless helpers for App ----

/// Take and clear the pending flag.
pub(crate) fn take_pending() -> bool {
    WHEEL_PENDING.swap(false, Ordering::SeqCst)
}

/// Peek without clearing.
pub(crate) fn peek_pending() -> bool {
    WHEEL_PENDING.load(Ordering::SeqCst)
}

/// Take and clear accumulated delta.
pub(crate) fn take_delta() -> i32 {
    WHEEL_DELTA.swap(0, Ordering::SeqCst)
}

/// Whether the cursor is over the tray icon's rect (with padding).
///
/// Returns `None` when the tray rect is unavailable.
#[cfg(windows)]
pub(crate) fn cursor_over_tray(wrapper: &crate::ui::tray::TrayWrapper) -> Option<bool> {
    // SAFETY: GetCursorPos writes to POINT out-param; rect() is tray-icon API.
    unsafe {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
        let mut pt = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut pt).is_err() {
            return Some(false);
        }
        let rect = wrapper.tray.rect()?;
        let x = f64::from(pt.x);
        let y = f64::from(pt.y);
        let pad = 16.0;
        Some(
            x >= rect.position.x - pad
                && x < rect.position.x + f64::from(rect.size.width) + pad
                && y >= rect.position.y - pad
                && y < rect.position.y + f64::from(rect.size.height) + pad,
        )
    }
}

#[cfg(not(windows))]
/// Non-Windows stub.
pub(crate) fn cursor_over_tray(_wrapper: &crate::ui::tray::TrayWrapper) -> Option<bool> {
    Some(false)
}

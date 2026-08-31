//! Windows low-level mouse wheel hook, encapsulated.
//!
//! The hook is installed lazily via [`WheelHook::install`] and automatically
//! removed on drop. Global atomics communicate wheel events to the main loop.

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

/// Accumulated wheel delta (WHEEL_DELTA is signed 120-per-notch).
static WHEEL_DELTA: AtomicI32 = AtomicI32::new(0);
/// Whether a wheel event is pending consumption.
static WHEEL_PENDING: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
static HOOK_HANDLE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
#[cfg(windows)]
static HOOK_REFCOUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

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
        #[allow(clippy::cast_possible_wrap, clippy::cast_lossless)]
        let delta = (info.mouseData >> 16) as u16 as i16 as i32;
        // Release ordering pairs with Acquire in consumer (take_pending/take_delta).
        WHEEL_DELTA.fetch_add(delta, Ordering::AcqRel);
        WHEEL_PENDING.store(true, Ordering::Release);
    }
    // SAFETY: CallNextHookEx is always safe to forward.
    unsafe { CallNextHookEx(None, n_code, w_param, l_param) }
}

/// RAII hook handle. Drop uninstalls the hook when last guard drops.
// The `PhantomData<*const ()>` makes it `!Send` — HHOOK is thread-affine.
pub struct WheelHook {
    _private: (),
    _marker: std::marker::PhantomData<*const ()>,
}

impl WheelHook {
    /// Install the low-level mouse hook.
    ///
    /// Returns `None` on Windows if `SetWindowsHookExW` fails.
    #[must_use]
    pub fn install() -> Option<Self> {
        #[cfg(windows)]
        {
            // Fast path: already installed.
            if HOOK_HANDLE.load(Ordering::Acquire) != 0 {
                HOOK_REFCOUNT.fetch_add(1, Ordering::AcqRel);
                // Double-check handle still valid after increment.
                if HOOK_HANDLE.load(Ordering::Acquire) == 0 {
                    HOOK_REFCOUNT.fetch_sub(1, Ordering::AcqRel);
                } else {
                    return Some(Self { _private: (), _marker: std::marker::PhantomData });
                }
            }
            // SAFETY: WH_MOUSE_LL is process-global, hook_proc has correct signature.
            let hook = unsafe {
                use windows::Win32::UI::WindowsAndMessaging::{SetWindowsHookExW, WH_MOUSE_LL};
                SetWindowsHookExW(WH_MOUSE_LL, Some(hook_proc), None, 0).ok()
            };
            if let Some(hook) = hook {
                let raw = hook.0 as usize;
                // Try to become the owner via compare_exchange.
                match HOOK_HANDLE.compare_exchange(0, raw, Ordering::AcqRel, Ordering::Acquire) {
                    Ok(_) => {
                        HOOK_REFCOUNT.store(1, Ordering::Release);
                        return Some(Self { _private: (), _marker: std::marker::PhantomData });
                    }
                    Err(existing) => {
                        // Another thread installed concurrently — use existing, leak our hook.
                        // SAFETY: we installed but lost race; unhook ours.
                        unsafe {
                            let _ =
                                windows::Win32::UI::WindowsAndMessaging::UnhookWindowsHookEx(hook);
                        }
                        if existing != 0 {
                            HOOK_REFCOUNT.fetch_add(1, Ordering::AcqRel);
                            return Some(Self { _private: (), _marker: std::marker::PhantomData });
                        }
                    }
                }
            }
            None
        }
        #[cfg(not(windows))]
        {
            Some(Self { _private: (), _marker: std::marker::PhantomData })
        }
    }
}

impl Drop for WheelHook {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            let prev = HOOK_REFCOUNT.fetch_sub(1, Ordering::AcqRel);
            if prev == 1 {
                let raw = HOOK_HANDLE.swap(0, Ordering::AcqRel);
                if raw != 0 {
                    // SAFETY: raw came from SetWindowsHookExW; balances exactly once.
                    unsafe {
                        let hook = windows::Win32::UI::WindowsAndMessaging::HHOOK(
                            raw as *mut std::ffi::c_void,
                        );
                        let _ = windows::Win32::UI::WindowsAndMessaging::UnhookWindowsHookEx(hook);
                    }
                }
            } else if prev == 0 {
                HOOK_REFCOUNT.store(0, Ordering::Release);
            }
        }
    }
}

// ---- stateless helpers for App ----

/// Take and clear the pending flag.
pub(crate) fn take_pending() -> bool {
    WHEEL_PENDING.swap(false, Ordering::AcqRel)
}

/// Peek without clearing.
pub(crate) fn peek_pending() -> bool {
    WHEEL_PENDING.load(Ordering::Acquire)
}

/// Take and clear accumulated delta.
pub(crate) fn take_delta() -> i32 {
    WHEEL_DELTA.swap(0, Ordering::AcqRel)
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

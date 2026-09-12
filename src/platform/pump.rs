//! Message-pump primitives. `app` owns the loop policy; this module owns the
//! Win32 calls (`ui` and `app` must not call Win32 directly).

/// Default wait timeout: periodic work (hook install, polling) still runs
/// while idle CPU stays negligible.
pub const PUMP_WAIT_MS: u32 = 200;

/// Fast wait while wheel events are pending, so volume tracking stays smooth.
const PUMP_WAIT_BUSY_MS: u32 = 8;

/// Drain pending Win32 messages without blocking.
pub(crate) fn pump_messages() {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, TranslateMessage, WM_HOTKEY,
        };
        loop {
            let mut msg = MSG::default();
            // SAFETY: `PeekMessageW` writes a plain POD `MSG` through the
            // raw out-pointer; no buffers escape.
            let pending = unsafe { PeekMessageW(&raw mut msg, None, 0, 0, PM_REMOVE).as_bool() };
            if !pending {
                break;
            }
            if msg.message == WM_HOTKEY {
                // `RegisterHotKey(None, ..)` binds to this thread and posts
                // `WM_HOTKEY` with a null window: `DispatchMessageW` would
                // drop it, so route the action to the hotkey module instead.
                crate::platform::hotkey::note_pending(msg.wParam.0 as i32);
                continue;
            }
            // SAFETY: `msg` was just written by `PeekMessageW` above.
            let _ = unsafe { TranslateMessage(&raw const msg) };
            // SAFETY: same freshly-drained `msg`; standard dispatch pair.
            unsafe { DispatchMessageW(&raw const msg) };
        }
    }
}

/// Block until input arrives or the timeout elapses.
pub(crate) fn wait_for_input(wheel_pending: bool) {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{
            MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, QS_ALLINPUT,
        };
        let timeout = if wheel_pending {
            PUMP_WAIT_BUSY_MS
        } else {
            PUMP_WAIT_MS
        };
        // SAFETY: MsgWaitForMultipleObjectsEx with an empty handle slice and
        // QS_ALLINPUT is safe to call on the UI thread.
        let _ = MsgWaitForMultipleObjectsEx(Some(&[]), timeout, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
    }
    #[cfg(not(windows))]
    std::thread::sleep(std::time::Duration::from_millis(u64::from(
        if wheel_pending { PUMP_WAIT_BUSY_MS } else { 24 },
    )));
}

/// Post `Quit`, ending the message loop.
pub(crate) fn quit() {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::PostQuitMessage;
        // SAFETY: Posts quit to the calling thread's queue; always safe.
        PostQuitMessage(0);
    }
}

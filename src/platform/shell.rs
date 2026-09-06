//! Shell helpers – deduplicates `TrayWrapper`'s two `ShellExecute` blocks.

/// Open `file` with `params` via `ShellExecuteW`.
///
/// # Errors
///
/// Returns `Err` when `ShellExecuteW` returns a value `<= 32` or when
/// `file`/`params` contain interior NUL bytes.
/// # Panics
///
/// Never panics — errors are returned.
#[cfg(windows)]
pub(crate) fn open_file(file: &str, params: Option<&str>) -> Result<(), String> {
    if file.contains('\0') {
        return Err("file contains interior NUL".into());
    }
    if let Some(p) = params {
        if p.contains('\0') {
            return Err("params contains interior NUL".into());
        }
    }
    // SAFETY: ShellExecuteW with null-terminated PCWSTRs living through the call.
    unsafe {
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        use windows::core::PCWSTR;
        let op: Vec<u16> = "open\0".encode_utf16().collect();
        let file_w: Vec<u16> = format!("{file}\0").encode_utf16().collect();
        let params_w: Option<Vec<u16>> = params.map(|p| format!("{p}\0").encode_utf16().collect());
        let res = ShellExecuteW(
            None,
            PCWSTR(op.as_ptr()),
            PCWSTR(file_w.as_ptr()),
            params_w.as_ref().map_or(PCWSTR::null(), |v| PCWSTR(v.as_ptr())),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
        if (res.0 as usize) <= 32 {
            Err(format!("ShellExecute failed: {}", res.0 as usize))
        } else {
            Ok(())
        }
    }
}

#[cfg(not(windows))]
/// Non-Windows stub.
pub(crate) fn open_file(_file: &str, _params: Option<&str>) -> Result<(), String> {
    Ok(())
}

/// Show a warning message box (centered on screen).
#[cfg(windows)]
pub(crate) fn show_error(msg: &str) {
    crate::platform::dialog::show_msgbox(msg);
}

#[cfg(not(windows))]
/// Non-Windows stub.
pub(crate) fn show_error(_msg: &str) {}

/// Pixel tolerance for the "already centered" check.
#[cfg(windows)]
const CENTER_TOLERANCE_PX: i32 = 2;
/// Hook lifetime bound; the watchdog posts `WM_QUIT` after this.
#[cfg(windows)]
const CENTER_HOOK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(6);
/// Grace period during which a drift is re-centered even for windows seen
/// centered before (covers dialogs that re-lay themselves out while
/// initializing). Afterwards a drift means the user dragged the window.
#[cfg(windows)]
const CENTER_RECONCILE_GRACE: std::time::Duration = std::time::Duration::from_millis(1500);
/// Quiet period with no further corrections after a match before the hook
/// exits early.
#[cfg(windows)]
const CENTER_QUIET: std::time::Duration = std::time::Duration::from_millis(1200);

/// Centered origin of `hwnd` within its nearest monitor work area.
///
/// Returns `None` when the window rect is unavailable or empty.
#[cfg(windows)]
fn centered_origin(hwnd: windows::Win32::Foundation::HWND) -> Option<(i32, i32)> {
    // SAFETY: Win32 FFI with valid HWND.
    unsafe {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::Graphics::Gdi::{
            GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
        };
        use windows::Win32::UI::WindowsAndMessaging::{
            GetSystemMetrics, GetWindowRect, SM_CXSCREEN, SM_CYSCREEN,
        };
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return None;
        }
        let w = rect.right - rect.left;
        let h = rect.bottom - rect.top;
        if w <= 0 || h <= 0 {
            return None;
        }
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut mi =
            MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        let (sw, sh, off_x, off_y) = if GetMonitorInfoW(monitor, &mut mi).as_bool() {
            (
                mi.rcWork.right - mi.rcWork.left,
                mi.rcWork.bottom - mi.rcWork.top,
                mi.rcWork.left,
                mi.rcWork.top,
            )
        } else {
            (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN), 0, 0)
        };
        Some((off_x + (sw - w) / 2, off_y + (sh - h) / 2))
    }
}

/// Move `hwnd` to center when it is off-center by more than a few pixels.
///
/// The window is moved directly and never hidden: the old hide-move-show
/// cycle made the window visibly disappear and reappear (flicker).
/// Returns `true` when the window is at center (within tolerance).
#[cfg(windows)]
fn ensure_centered(hwnd: windows::Win32::Foundation::HWND) -> bool {
    // SAFETY: Win32 FFI with valid HWND.
    unsafe {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetWindowRect, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos,
        };
        let Some((tx, ty)) = centered_origin(hwnd) else {
            return true;
        };
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return true;
        }
        if (rect.left - tx).abs() <= CENTER_TOLERANCE_PX
            && (rect.top - ty).abs() <= CENTER_TOLERANCE_PX
        {
            return true;
        }
        let _ = SetWindowPos(hwnd, None, tx, ty, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
        false
    }
}

/// Visible, non-minimized top-level windows whose title contains any of
/// `keywords` (case-insensitive).
///
/// Invisible or minimized windows are skipped: hidden helper windows must
/// never count as a match, otherwise the worker would stop before the real
/// dialog appears, and minimized windows must not be moved or restored.
#[cfg(windows)]
fn visible_matching_windows(keywords: &[String]) -> Vec<windows::Win32::Foundation::HWND> {
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowTextW, IsIconic, IsWindowVisible,
    };
    use windows_core::BOOL;

    struct Ctx {
        keywords: Vec<String>,
        out: Vec<HWND>,
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: lparam is a valid *mut Ctx from caller, lives for EnumWindows duration.
        let ctx = unsafe { &mut *(lparam.0 as *mut Ctx) };
        // SAFETY: IsWindowVisible/IsIconic with valid HWND.
        if !unsafe { IsWindowVisible(hwnd) }.as_bool() || unsafe { IsIconic(hwnd) }.as_bool() {
            return BOOL(1);
        }
        // Skip windows with no title.
        let len = usize::try_from(unsafe { GetWindowTextLengthW(hwnd) }).unwrap_or(0);
        if len == 0 {
            return BOOL(1);
        }
        let mut buf = vec![0u16; len + 1];
        let read = usize::try_from(unsafe { GetWindowTextW(hwnd, &mut buf) }).unwrap_or(0);
        if read == 0 {
            return BOOL(1);
        }
        let lower = String::from_utf16_lossy(&buf[..read]).to_lowercase();
        if ctx.keywords.iter().any(|kw| lower.contains(kw)) {
            ctx.out.push(hwnd);
        }
        BOOL(1)
    }

    let mut ctx =
        Ctx { keywords: keywords.iter().map(|k| k.to_lowercase()).collect(), out: Vec::new() };
    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(&mut ctx as *mut _ as isize));
    }
    ctx.out
}

/// Per-worker state shared with the hook callback.
///
/// The callback runs on the installing thread (via its message loop), so a
/// thread-local is enough and avoids any cross-thread locking.
#[cfg(windows)]
struct CenterHookCtx {
    keywords: Vec<String>,
    /// Windows already centered: `(hwnd, first_centered_at)`. Within
    /// `CENTER_RECONCILE_GRACE` their drift is corrected; afterwards they are
    /// released (user-dragged windows are left alone).
    tracked: Vec<(isize, std::time::Instant)>,
    started: std::time::Instant,
    last_action: std::time::Instant,
    matched: bool,
}

#[cfg(windows)]
thread_local! {
    static CENTER_HOOK_CTX: std::cell::RefCell<Option<CenterHookCtx>> =
        const { std::cell::RefCell::new(None) };
}

/// Read the title of a visible, non-minimized window (lowercased, empty when
/// the window is hidden/minimized/untitled or the read fails).
#[cfg(windows)]
fn visible_window_title(hwnd: windows::Win32::Foundation::HWND) -> String {
    // SAFETY: Win32 FFI with valid HWND.
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetWindowTextLengthW, GetWindowTextW, IsIconic, IsWindowVisible,
        };
        if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return String::new();
        }
        let len = usize::try_from(GetWindowTextLengthW(hwnd)).unwrap_or(0);
        if len == 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len + 1];
        let read = usize::try_from(GetWindowTextW(hwnd, &mut buf)).unwrap_or(0);
        if read == 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buf[..read]).to_lowercase()
    }
}

/// `WinEventProc` for the centering hook.
///
/// Runs on the installing thread whenever a window is created, shown or
/// changes position. Matching windows are centered immediately — at
/// `EVENT_OBJECT_SHOW` time the window has not painted its first frame yet,
/// so it appears already centered instead of being visibly moved.
#[cfg(windows)]
unsafe extern "system" fn center_win_event_proc(
    _hook: windows::Win32::UI::Accessibility::HWINEVENTHOOK,
    event: u32,
    hwnd: windows::Win32::Foundation::HWND,
    idobject: i32,
    _idchild: i32,
    _idthread: u32,
    _dwmseventtime: u32,
) {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::{
        EVENT_OBJECT_CREATE, EVENT_OBJECT_SHOW, OBJID_WINDOW, PostThreadMessageW, WM_QUIT,
    };

    let finish = |quit_now: bool| {
        // SAFETY: posting to our own thread id; WM_QUIT just ends the loop.
        unsafe {
            let _ = PostThreadMessageW(GetCurrentThreadId(), WM_QUIT, WPARAM(0), LPARAM(0));
        }
        if quit_now {
            CENTER_HOOK_CTX.with_borrow_mut(|slot| *slot = None);
        }
    };

    // Timeout / early-exit bookkeeping must run for every event.
    let expired = CENTER_HOOK_CTX.with_borrow(|slot| {
        slot.as_ref().is_none_or(|ctx| {
            let now = std::time::Instant::now();
            now.duration_since(ctx.started) >= CENTER_HOOK_TIMEOUT
                || (ctx.matched && now.duration_since(ctx.last_action) >= CENTER_QUIET)
        })
    });
    if expired {
        finish(false);
        return;
    }
    // Only top-level window objects are relevant.
    if hwnd.is_invalid() || idobject != OBJID_WINDOW.0 {
        return;
    }

    CENTER_HOOK_CTX.with_borrow_mut(|slot| {
        let Some(ctx) = slot.as_mut() else { return };
        let raw = hwnd.0 as isize;
        let now = std::time::Instant::now();
        // Known window: reconcile self-re-layout drift within the grace
        // window, then release it (further moves are user drags).
        if let Some(pos) = ctx.tracked.iter().position(|(h, _)| *h == raw) {
            if now.duration_since(ctx.tracked[pos].1) >= CENTER_RECONCILE_GRACE {
                ctx.tracked.remove(pos);
            } else if !ensure_centered(hwnd) {
                ctx.last_action = now;
            }
            return;
        }
        // New window: react to creation/show only, not to every position
        // change of unrelated windows.
        if event != EVENT_OBJECT_SHOW && event != EVENT_OBJECT_CREATE {
            return;
        }
        let title = visible_window_title(hwnd);
        if !title.is_empty() && ctx.keywords.iter().any(|kw| title.contains(kw)) {
            if !ensure_centered(hwnd) {
                ctx.last_action = now;
            }
            ctx.tracked.push((raw, now));
            ctx.matched = true;
            ctx.last_action = now;
        }
    });
}

/// Center windows matching `keywords` from a background thread.
///
/// Event-driven instead of polling: a `SetWinEventHook`
/// (`WINEVENT_OUTOFCONTEXT`) delivers window create/show events to this
/// thread's message loop, and matching windows are moved to center inside
/// the callback — typically before their first frame is painted, so the
/// dialog *opens* centered instead of being visibly moved there. Windows
/// that re-layout themselves while initializing are corrected within a
/// grace period; later drifts are treated as user drags and left alone.
#[cfg(windows)]
pub(crate) fn spawn_center_for_keywords(keywords: &'static [&'static str]) {
    std::thread::spawn(move || {
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        use windows::Win32::System::Threading::GetCurrentThreadId;
        use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
        use windows::Win32::UI::WindowsAndMessaging::{
            DispatchMessageW, EVENT_OBJECT_CREATE, EVENT_OBJECT_LOCATIONCHANGE, GetMessageW, MSG,
            PostThreadMessageW, TranslateMessage, WINEVENT_OUTOFCONTEXT, WM_QUIT,
        };

        let thread_id = unsafe { GetCurrentThreadId() };
        // Seed state and sweep windows that may already be visible.
        CENTER_HOOK_CTX.with_borrow_mut(|slot| {
            let mut ctx = CenterHookCtx {
                keywords: keywords.iter().map(|k| k.to_lowercase()).collect(),
                tracked: Vec::new(),
                started: std::time::Instant::now(),
                last_action: std::time::Instant::now(),
                matched: false,
            };
            for hwnd in visible_matching_windows(&ctx.keywords) {
                if !ensure_centered(hwnd) {
                    ctx.last_action = std::time::Instant::now();
                }
                ctx.tracked.push((hwnd.0 as isize, std::time::Instant::now()));
                ctx.matched = true;
            }
            *slot = Some(ctx);
        });
        // Watchdog: guarantee the message loop ends even if no events fire.
        std::thread::spawn(move || {
            std::thread::sleep(CENTER_HOOK_TIMEOUT);
            // SAFETY: posting WM_QUIT to the hook thread; harmless if it already quit.
            unsafe {
                let _ = PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        });
        // SAFETY: hook proc has the required signature and outlives the loop;
        // the message loop below keeps the hook thread pumping events.
        let hook = unsafe {
            SetWinEventHook(
                EVENT_OBJECT_CREATE,
                EVENT_OBJECT_LOCATIONCHANGE,
                None,
                Some(center_win_event_proc),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            )
        };
        if !hook.0.is_null() {
            // SAFETY: standard message loop; GetMessageW returns FALSE on WM_QUIT.
            unsafe {
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                let _ = UnhookWinEvent(HWINEVENTHOOK(hook.0));
            }
        }
        CENTER_HOOK_CTX.with_borrow_mut(|slot| *slot = None);
    });
}

#[cfg(not(windows))]
pub(crate) fn spawn_center_for_keywords(_keywords: &'static [&'static str]) {}

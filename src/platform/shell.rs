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
        use windows::core::PCWSTR;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
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

#[cfg(windows)]
fn center_hwnd(hwnd: windows::Win32::Foundation::HWND) {
    // SAFETY: Win32 FFI with valid HWND.
    unsafe {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
        use windows::Win32::UI::WindowsAndMessaging::{
            GetSystemMetrics, GetWindowRect, SetWindowPos, SM_CXSCREEN, SM_CYSCREEN, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
        };
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return;
        }
        let w = rect.right - rect.left;
        let h = rect.bottom - rect.top;
        if w <= 0 || h <= 0 {
            return;
        }
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let (sw, sh, off_x, off_y) = if GetMonitorInfoW(monitor, &mut mi).as_bool() {
            (mi.rcWork.right - mi.rcWork.left, mi.rcWork.bottom - mi.rcWork.top, mi.rcWork.left, mi.rcWork.top)
        } else {
            (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN), 0, 0)
        };
        let x = off_x + (sw - w) / 2;
        let y = off_y + (sh - h) / 2;
        let _ = SetWindowPos(hwnd, None, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
    }
}

#[cfg(windows)]
fn center_matching_window(keywords: &[&str]) -> bool {
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowTextW, IsWindowVisible, ShowWindow, SW_HIDE, SW_SHOW,
    };
    use windows_core::BOOL;

    struct Ctx<'a> {
        keywords: &'a [&'a str],
        found: bool,
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: lparam is a valid *mut Ctx from caller, lives for EnumWindows duration.
        let ctx = unsafe { &mut *(lparam.0 as *mut Ctx<'_>) };
        // Skip windows with no title
        let len = unsafe { GetWindowTextLengthW(hwnd) };
        if len == 0 {
            return BOOL(1);
        }
        let mut buf = vec![0u16; (len as usize) + 1];
        let read = unsafe { GetWindowTextW(hwnd, &mut buf) };
        if read == 0 {
            return BOOL(1);
        }
        let text = String::from_utf16_lossy(&buf[..read as usize]);
        let lower = text.to_lowercase();
        for kw in ctx.keywords {
            if lower.contains(&kw.to_lowercase()) {
                // If window is already visible, hide -> move -> show to avoid flash at old pos.
                // If not yet visible, just move so it first appears centered.
                let was_visible = unsafe { IsWindowVisible(hwnd).as_bool() };
                if was_visible {
                    unsafe { let _ = ShowWindow(hwnd, SW_HIDE); }
                }
                center_hwnd(hwnd);
                if was_visible {
                    unsafe { let _ = ShowWindow(hwnd, SW_SHOW); }
                }
                ctx.found = true;
                break;
            }
        }
        BOOL(1)
    }

    let mut ctx = Ctx { keywords, found: false };
    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(&mut ctx as *mut _ as isize));
    }
    ctx.found
}

#[cfg(windows)]
pub(crate) fn spawn_center_for_keywords(keywords: &'static [&'static str]) {
    std::thread::spawn(move || {
        // Fast polling (10ms) to catch window before it paints at default position.
        // 300 * 10ms ≈ 3s timeout, first check immediate (no initial sleep).
        for i in 0..300 {
            if center_matching_window(keywords) {
                // Ensure late-show window also centered
                std::thread::sleep(std::time::Duration::from_millis(50));
                center_matching_window(keywords);
                break;
            }
            // First iteration already checked, now sleep
            if i == 0 {
                // Give the target process a tiny head start without waiting 10ms
                std::thread::sleep(std::time::Duration::from_millis(10));
            } else {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
    });
}

#[cfg(not(windows))]
pub(crate) fn spawn_center_for_keywords(_keywords: &'static [&'static str]) {}

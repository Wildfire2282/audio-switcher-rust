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

/// Show a warning message box.
#[cfg(windows)]
pub(crate) fn show_error(msg: &str) {
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONWARNING, MB_OK};
    // SAFETY: MessageBoxW with null-terminated PCWSTRs.
    unsafe {
        let wide: Vec<u16> = format!("{msg}\0").encode_utf16().collect();
        let title: Vec<u16> = "Audio Switcher\0".encode_utf16().collect();
        MessageBoxW(None, PCWSTR(wide.as_ptr()), PCWSTR(title.as_ptr()), MB_OK | MB_ICONWARNING);
    }
}

#[cfg(not(windows))]
/// Non-Windows stub.
pub(crate) fn show_error(_msg: &str) {}

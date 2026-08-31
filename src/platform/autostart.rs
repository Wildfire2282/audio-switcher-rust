//! Auto-launch (autostart) helpers.

use std::path::PathBuf;

use auto_launch::{AutoLaunch, WindowsEnableMode};

/// Returns the current executable path, if determinable.
#[must_use]
pub(crate) fn get_exe_path() -> Option<PathBuf> {
    std::env::current_exe().ok()
}

fn autolaunch_for_current_exe() -> Option<AutoLaunch> {
    let exe = get_exe_path()?;
    let exe_str = exe.to_string_lossy().to_string();
    Some(AutoLaunch::new("AudioSwitcher", &exe_str, WindowsEnableMode::CurrentUser, &[] as &[&str]))
}

/// Enable or disable auto-launch at login.
///
/// # Errors
///
/// Returns `Err` with a diagnostic string when the underlying registry
/// operation fails.
pub(crate) fn set_autostart(enable: bool) -> Result<(), String> {
    let auto = autolaunch_for_current_exe().ok_or_else(|| "cannot get exe path".to_string())?;
    if enable {
        auto.enable().map_err(|e| e.to_string())
    } else {
        auto.disable().map_err(|e| e.to_string())
    }
}

/// Whether auto-launch is currently enabled.
#[must_use]
pub(crate) fn is_autostart_enabled() -> bool {
    autolaunch_for_current_exe().and_then(|auto| auto.is_enabled().ok()).unwrap_or(false)
}

use auto_launch::{AutoLaunch, WindowsEnableMode};

pub fn get_exe_path() -> Option<String> {
    std::env::current_exe().ok().map(|p| p.to_string_lossy().to_string())
}

pub fn set_autostart(enable: bool) -> Result<(), String> {
    let exe = get_exe_path().ok_or("cannot get exe path")?;
    let auto = AutoLaunch::new("AudioSwitcher", &exe, WindowsEnableMode::CurrentUser, &[] as &[&str]);
    if enable { auto.enable().map_err(|e| e.to_string()) } else { auto.disable().map_err(|e| e.to_string()) }
}

pub fn is_autostart_enabled() -> bool {
    if let Some(exe) = get_exe_path() {
        let auto = AutoLaunch::new("AudioSwitcher", &exe, WindowsEnableMode::CurrentUser, &[] as &[&str]);
        auto.is_enabled().unwrap_or(false)
    } else {
        false
    }
}

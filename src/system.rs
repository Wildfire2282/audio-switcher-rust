use auto_launch::{AutoLaunch, WindowsEnableMode};
use single_instance::SingleInstance;

pub struct SingleInstanceGuard {
    _instance: SingleInstance,
}

impl SingleInstanceGuard {
    pub fn new(name: &str) -> Option<Self> {
        let instance = SingleInstance::new(name).ok()?;
        if !instance.is_single() {
            return None;
        }
        Some(Self {
            _instance: instance,
        })
    }
}

pub fn get_exe_path() -> Option<String> {
    std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().to_string())
}

pub fn set_autostart(enable: bool) -> Result<(), String> {
    let exe = get_exe_path().ok_or("cannot get exe path")?;
    let auto = AutoLaunch::new(
        "AudioSwitcher",
        &exe,
        WindowsEnableMode::CurrentUser,
        &[] as &[&str],
    );
    if enable {
        auto.enable().map_err(|e| e.to_string())
    } else {
        auto.disable().map_err(|e| e.to_string())
    }
}

pub fn is_autostart_enabled() -> bool {
    if let Some(exe) = get_exe_path() {
        let auto = AutoLaunch::new(
            "AudioSwitcher",
            &exe,
            WindowsEnableMode::CurrentUser,
            &[] as &[&str],
        );
        auto.is_enabled().unwrap_or(false)
    } else {
        false
    }
}

#[cfg(windows)]
pub fn show_autostart_error() {
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONWARNING, MB_OK};
    unsafe {
        let msg: Vec<u16> = "设置开机自启失败\0".encode_utf16().collect();
        let title: Vec<u16> = "Audio Switcher\0".encode_utf16().collect();
        MessageBoxW(
            None,
            PCWSTR(msg.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONWARNING,
        );
    }
}

#[cfg(not(windows))]
pub fn show_autostart_error() {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn single_instance_name() {
        let g = SingleInstanceGuard::new("audio-switcher-test-single-instance-unique-12345");
        assert!(g.is_some());
    }
}

use std::time::Instant;

#[cfg(windows)]
static DARK_MODE_CACHE: std::sync::LazyLock<parking_lot::Mutex<(bool, Instant)>> =
    std::sync::LazyLock::new(|| parking_lot::Mutex::new((raw_is_dark_mode(), Instant::now())));

#[cfg(windows)]
fn raw_is_dark_mode() -> bool {
    use windows::Win32::System::Registry::{
        RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ, REG_NONE, REG_VALUE_TYPE,
    };
    use windows::Win32::System::Registry::HKEY;
    use windows::core::PCWSTR;
    unsafe {
        let subkey: Vec<u16> =
            "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0"
                .encode_utf16()
                .collect();
        let mut hkey = HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            Some(0),
            KEY_READ,
            &mut hkey,
        )
        .is_err()
        {
            return false;
        }
        for val in ["SystemUsesLightTheme\0", "AppsUseLightTheme\0"] {
            let valname: Vec<u16> = val.encode_utf16().collect();
            let mut data: u32 = 0;
            let mut len = std::mem::size_of::<u32>() as u32;
            let mut ty: REG_VALUE_TYPE = REG_NONE;
            let res = RegQueryValueExW(
                hkey,
                PCWSTR(valname.as_ptr()),
                None,
                Some(&mut ty),
                Some(&mut data as *mut u32 as *mut u8),
                Some(&mut len),
            );
            if res.is_ok() {
                let _ = windows::Win32::System::Registry::RegCloseKey(hkey);
                return data == 0;
            }
        }
        let _ = windows::Win32::System::Registry::RegCloseKey(hkey);
        false
    }
}

#[cfg(windows)]
pub fn is_dark_mode() -> bool {
    let now = Instant::now();
    {
        let cache = DARK_MODE_CACHE.lock();
        if now.duration_since(cache.1) < std::time::Duration::from_millis(2000) {
            return cache.0;
        }
    }
    let fresh = raw_is_dark_mode();
    let mut cache = DARK_MODE_CACHE.lock();
    if now.duration_since(cache.1) >= std::time::Duration::from_millis(2000) {
        *cache = (fresh, now);
        fresh
    } else {
        cache.0
    }
}

#[cfg(windows)]
pub fn invalidate_dark_mode_cache() {
    let mut cache = DARK_MODE_CACHE.lock();
    *cache = (raw_is_dark_mode(), Instant::now());
}

#[cfg(not(windows))]
pub fn is_dark_mode() -> bool {
    false
}

#[cfg(not(windows))]
pub fn invalidate_dark_mode_cache() {}

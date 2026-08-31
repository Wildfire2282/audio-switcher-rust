use tray_icon::Icon;

static ICON_CACHE: std::sync::LazyLock<parking_lot::Mutex<std::collections::HashMap<bool, Icon>>> =
    std::sync::LazyLock::new(|| parking_lot::Mutex::new(std::collections::HashMap::new()));

/// Single-key cache; `_is_dark` kept for call-site compatibility but ignored.
/// Pre-rendered on colored background works on both light/dark taskbars.
pub fn make_icon(muted: bool, _is_dark: bool) -> Icon {
    if let Some(cached) = ICON_CACHE.lock().get(&muted).cloned() {
        return cached;
    }
    let rgba: &[u8] = if muted {
        include_bytes!("../../icons/tray_muted_red.rgba")
    } else {
        include_bytes!("../../icons/tray_unmuted_bg.rgba")
    };
    let icon = Icon::from_rgba(rgba.to_vec(), 32, 32).expect("tray icon rgba invalid");
    ICON_CACHE.lock().insert(muted, icon.clone());
    icon
}

/// New minimal API – prefer this.
#[allow(dead_code)]
pub fn make_icon_simple(muted: bool) -> Icon {
    make_icon(muted, false)
}

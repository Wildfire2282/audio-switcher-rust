use tray_icon::Icon;

static ICON_CACHE: std::sync::LazyLock<parking_lot::Mutex<std::collections::HashMap<(bool, bool), Icon>>> =
    std::sync::LazyLock::new(|| parking_lot::Mutex::new(std::collections::HashMap::new()));

pub fn make_icon(muted: bool, _is_dark: bool) -> Icon {
    let key = (muted, false);
    if let Some(cached) = ICON_CACHE.lock().get(&key).cloned() {
        return cached;
    }
    let rgba: &[u8] = if muted {
        include_bytes!("../../icons/tray_muted_red.rgba")
    } else {
        include_bytes!("../../icons/tray_unmuted_bg.rgba")
    };
    let icon = Icon::from_rgba(rgba.to_vec(), 32, 32).expect("tray icon rgba invalid");
    let mut cache = ICON_CACHE.lock();
    cache.insert(key, icon.clone());
    cache.insert((muted, true), icon.clone());
    icon
}

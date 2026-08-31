//! Tray icon creation with caching.
//!
//! Icons are 32×32 RGBA loaded from `icons/*.rgba`.

use std::sync::LazyLock;

use parking_lot::Mutex;
use tray_icon::Icon;

/// Cached icons — index 0 = unmuted, 1 = muted.
static ICON_CACHE: LazyLock<Mutex<[Option<Icon>; 2]>> = LazyLock::new(|| Mutex::new([None, None]));

/// Create (or fetch from cache) the tray icon for `muted`.
#[must_use]
pub fn make_icon(muted: bool) -> Icon {
    let idx = usize::from(muted);
    if let Some(cached) = ICON_CACHE.lock()[idx].clone() {
        return cached;
    }
    let rgba: &[u8] = if muted {
        include_bytes!("../../icons/tray_muted_red.rgba")
    } else {
        include_bytes!("../../icons/tray_unmuted_bg.rgba")
    };
    // Pre-validated asset: expect only on corrupt build artefacts.
    let icon = Icon::from_rgba(rgba.to_vec(), 32, 32).expect("tray icon rgba invalid");
    ICON_CACHE.lock()[idx] = Some(icon.clone());
    icon
}

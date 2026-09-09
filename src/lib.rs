//! Audio Switcher — Windows tray audio switcher.
//!
//! Library crate holds all reusable logic; `main.rs` installs crash reporting
//! (file sink + panic hook) then runs guards → `App::run`.
//!
//! # Architecture
//! - `config` — strongly typed configuration and persistence
//! - `audio` — `AudioBackend` abstraction
//! - `platform` — Windows platform wrappers
//! - `ui` — tray UI
//! - `app` — runtime
//!
//! Engineering standard: `docs/unified-scheme.md` (generic tray-app development standard).
#![warn(missing_docs)]
#![warn(unsafe_op_in_unsafe_fn)]
// Baseline-inherent duplicates (single-instance 0.3.3 pulls thiserror 1/syn 1;
// tray-icon's tree pulls old unix-gated nix/memoffset/bitflags/miniz_oxide):
// locked by §1, upgrades by separate decision. Re-check on every baseline
// bump with `cargo tree -i <crate>`; new direct-dep duplicates stay denied.
#![allow(clippy::multiple_crate_versions)]
// `pub` below is the minimum the `main` binary and doctests need; everything
// else defaults to `pub(crate)` (no `prelude` module: only two import sites
// ever used it, a glob re-export is not worth the indirection).
pub mod app;
pub(crate) mod audio;
pub mod config;
pub(crate) mod platform;
pub(crate) mod ui;
// Curated re-exports for the binary entry point (`main` is a separate crate,
// so anything it touches is `pub` with this justification).
pub use config::{AppConfig, Lang};
pub use platform::dialog::show_critical;
pub use platform::logging::init as init_crash_reporting;
pub use platform::{ComError, ComGuard, InstanceError, SingleInstanceGuard};

/// Canonical tool id (kebab-case). Single source for the mutex name, the
/// config/log directory names, and the autostart display-name derivation.
pub const TOOL_ID: &str = "audio-switcher";

/// PascalCase display name, derived from [`TOOL_ID`]. Used for the autostart
/// registry value, dialog titles, and the menu title only.
pub const TOOL_DISPLAY_NAME: &str = "AudioSwitcher";

/// Tool release homepage. Single definition per crate; the About menu item
/// opens exactly this URL (validated through [`platform::shell::Url`]).
pub const ABOUT_URL: &str = "https://github.com/Wildfire2282/audio-switcher";

/// Canonical single-instance mutex id: `{kebab}-single-instance-v1`.
#[must_use]
pub fn single_instance_id() -> String {
    format!("{TOOL_ID}-single-instance-v1")
}

/// Derive the PascalCase display name from a kebab-case tool id
/// (`"audio-switcher"` → `"AudioSwitcher"`).
#[must_use]
pub fn display_name_for(kebab: &str) -> String {
    kebab
        .split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                // Uppercase may expand (e.g. ligatures); lowercase the rest.
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
                None => String::new(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_id_mapping() {
        // Mutex, autostart display name, and config dir all derive from TOOL_ID.
        assert_eq!(TOOL_ID, "audio-switcher");
        assert_eq!(single_instance_id(), "audio-switcher-single-instance-v1");
        assert_eq!(display_name_for(TOOL_ID), TOOL_DISPLAY_NAME);
        assert_eq!(TOOL_DISPLAY_NAME, "AudioSwitcher");
    }

    #[test]
    fn about_url_single_const() {
        // Prefix locked: About always lands on the tool release homepage.
        assert!(ABOUT_URL.starts_with("https://github.com/Wildfire2282/"));
        // Scheme validated through the same gate the menu handler uses.
        let url: crate::platform::shell::Url = ABOUT_URL.parse().expect("ABOUT_URL valid");
        assert_eq!(url.as_str(), ABOUT_URL);
    }
}

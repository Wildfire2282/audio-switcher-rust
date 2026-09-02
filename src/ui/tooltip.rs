//! Tooltip formatting for the tray icon.

use crate::audio::AudioDevice;
use crate::config::Lang;
use crate::ui::i18n::tr;
use crate::ui::text::{MAX_LABEL_CHARS, truncate_label};

/// Build the tray tooltip text.
///
/// - Muted → localized "Muted".
/// - Otherwise `"Device - 62%"` truncated to 60 characters, control chars sanitized.
#[must_use]
pub fn format_tooltip(device: Option<&AudioDevice>, volume: u32, mute: bool, lang: Lang) -> String {
    if mute {
        tr("muted", lang)
    } else if let Some(d) = device {
        // truncate_label sanitizes internally; don't pre-sanitize (avoids double allocation).
        let base = format!("{} - {volume}%", d.name);
        truncate_label(&base, MAX_LABEL_CHARS)
    } else {
        format!("{volume}%")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::AudioDevice;
    use crate::config::Lang;

    #[test]
    fn tooltip_format() {
        let dev = AudioDevice { id: "a".into(), name: "Realtek Speaker".into() };
        assert_eq!(format_tooltip(Some(&dev), 62, false, Lang::Zh), "Realtek Speaker - 62%");
        assert_eq!(format_tooltip(Some(&dev), 0, true, Lang::Zh), "静音");
        assert_eq!(format_tooltip(Some(&dev), 0, true, Lang::En), "Muted");
        let long = AudioDevice { id: "a".into(), name: "A".repeat(100) };
        let tip = format_tooltip(Some(&long), 50, false, Lang::Zh);
        assert!(tip.chars().count() <= 60);
    }

    #[test]
    fn tooltip_sanitizes_newline() {
        let dev = AudioDevice { id: "a".into(), name: "Speaker\nInjected".into() };
        let tip = format_tooltip(Some(&dev), 50, false, Lang::En);
        assert!(!tip.contains('\n'));
        assert!(tip.contains("Speaker Injected"));
    }

    #[test]
    fn clamp_via_config() {
        use crate::config::clamp_volume;
        let mut cfg = crate::config::AppConfig {
            volume_limit_enabled: true,
            volume_limit: 30,
            ..Default::default()
        };
        assert_eq!(clamp_volume(80, &cfg), 30);
        cfg.volume_limit_enabled = false;
        assert_eq!(clamp_volume(80, &cfg), 80);
        // Disabled still caps to 100
        assert_eq!(clamp_volume(150, &cfg), 100);
    }
}

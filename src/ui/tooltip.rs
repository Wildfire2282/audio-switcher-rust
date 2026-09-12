//! Tooltip formatting for the tray icon.

use crate::audio::AudioDevice;
use crate::config::Lang;
use crate::ui::i18n::tr;
use crate::ui::text::{MAX_LABEL_CHARS, truncate_label};

/// Build the tray tooltip text.
///
/// One line, always naming the default device when there is one:
/// `"Device - 62%"`, or `"Device - Muted"` while muted (EarTrumpet's tray
/// tooltip keeps the device next to the state). Abbreviate, never wrap
/// (64-char Win32 budget, shared with menu labels via [`MAX_LABEL_CHARS`]).
pub fn format_tooltip(device: Option<&AudioDevice>, volume: u32, mute: bool, lang: Lang) -> String {
    let suffix = if mute {
        tr("muted", lang)
    } else {
        format!("{volume}%")
    };
    match device {
        // Pre-truncate the name to the budget minus the exact suffix width, so
        // the single format! below cannot exceed it: no 200-char temp string,
        // no second counting pass (`saturating_sub` guards tiny budgets).
        Some(d) => {
            let suffix_len = 3 + suffix.chars().count();
            let name = truncate_label(&d.name, MAX_LABEL_CHARS.saturating_sub(suffix_len));
            format!("{name} - {suffix}")
        }
        None => suffix,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::AudioDevice;
    use crate::config::Lang;

    #[test]
    fn tooltip_format() {
        let dev = AudioDevice {
            id: "a".into(),
            name: "Realtek Speaker".into(),
        };
        assert_eq!(
            format_tooltip(Some(&dev), 62, false, Lang::Zh),
            "Realtek Speaker - 62%"
        );
        // Muted keeps the device name (state replaces the volume).
        assert_eq!(
            format_tooltip(Some(&dev), 62, true, Lang::Zh),
            "Realtek Speaker - 静音"
        );
        assert_eq!(
            format_tooltip(Some(&dev), 62, true, Lang::En),
            "Realtek Speaker - Muted"
        );
        // No enumeration/default: the state is all that is left to show.
        assert_eq!(format_tooltip(None, 62, false, Lang::En), "62%");
        assert_eq!(format_tooltip(None, 62, true, Lang::En), "Muted");
        let long = AudioDevice {
            id: "a".into(),
            name: "A".repeat(100),
        };
        let tip = format_tooltip(Some(&long), 50, false, Lang::Zh);
        assert!(tip.chars().count() <= 64);
    }
    #[test]
    fn tooltip_never_exceeds_64_char_budget() {
        // Worst case: longest plausible name at max volume, unmuted.
        let long = AudioDevice {
            id: "a".into(),
            name: "X".repeat(200),
        };
        for lang in [Lang::System, Lang::Zh, Lang::En] {
            let tip = format_tooltip(Some(&long), 100, false, lang);
            assert!(tip.chars().count() <= 64, "budget exceeded: {tip}");
        }
        let muted = format_tooltip(Some(&long), 100, true, Lang::Zh);
        assert!(muted.chars().count() <= 64);
    }

    #[test]
    fn tooltip_sanitizes_newline() {
        let dev = AudioDevice {
            id: "a".into(),
            name: "Speaker\nInjected".into(),
        };
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

use crate::audio::AudioDevice;
use crate::ui::i18n::tr;

pub fn format_tooltip(device: Option<&AudioDevice>, volume: u32, mute: bool, lang: &str) -> String {
    if mute {
        tr("muted", lang)
    } else if let Some(d) = device {
        let base = format!("{} - {}%", d.name, volume);
        truncate(&base)
    } else {
        format!("{}%", volume)
    }
}

fn truncate(s: &str) -> String {
    if s.chars().count() > 60 {
        let t: String = s.chars().take(58).collect();
        format!("{}…", t)
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::AudioDevice;
    #[test]
    fn tooltip_format() {
        let dev = AudioDevice { id: "a".into(), name: "Realtek Speaker".into() };
        assert_eq!(format_tooltip(Some(&dev), 62, false, "zh"), "Realtek Speaker - 62%");
        assert_eq!(format_tooltip(Some(&dev), 0, true, "zh"), "静音");
        assert_eq!(format_tooltip(Some(&dev), 0, true, "en"), "Muted");
        let long = AudioDevice { id: "a".into(), name: "A".repeat(100) };
        let tip = format_tooltip(Some(&long), 50, false, "zh");
        assert!(tip.chars().count() <= 60);
    }
    #[test]
    fn clamp_via_config() {
        let mut cfg = crate::config::AppConfig { volume_limit_enabled: true, volume_limit: 30, ..Default::default() };
        use crate::config::clamp_volume;
        assert_eq!(clamp_volume(80, &cfg), 30);
        cfg.volume_limit_enabled = false;
        assert_eq!(clamp_volume(80, &cfg), 80);
    }
}

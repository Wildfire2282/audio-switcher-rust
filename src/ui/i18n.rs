//! Internationalisation helpers.
//!
//! `tr` translates a message key for the given [`Lang`].

use crate::config::Lang;

/// Translate `key` for `lang`.
///
/// Unknown keys are returned verbatim, which keeps menus debuggable.
#[must_use]
pub fn tr(key: &str, lang: Lang) -> String {
    let zh = lang.is_zh();
    match key {
        "mute" => {
            if zh {
                "全局静音".into()
            } else {
                "Mute".into()
            }
        }
        "volume_limit" => {
            if zh {
                "音量上限".into()
            } else {
                "Volume Limit".into()
            }
        }
        "experimental" => {
            if zh {
                "实验性功能".into()
            } else {
                "Experimental".into()
            }
        }
        "enabled" => {
            if zh {
                "启用".into()
            } else {
                "Enabled".into()
            }
        }
        "wheel_accel" => {
            if zh {
                "滚轮加速".into()
            } else {
                "Wheel Acceleration".into()
            }
        }
        "open_mixer" => {
            if zh {
                "打开音量合成器".into()
            } else {
                "Open Volume Mixer".into()
            }
        }
        "open_sound" => {
            if zh {
                "打开声音设置".into()
            } else {
                "Open Sound Settings".into()
            }
        }
        "autostart" => {
            if zh {
                "开机自启".into()
            } else {
                "Auto Launch".into()
            }
        }
        "about" => {
            if zh {
                "关于".into()
            } else {
                "About".into()
            }
        }
        "exit" => {
            if zh {
                "退出".into()
            } else {
                "Exit".into()
            }
        }
        "chinese" => "中文".into(),
        "english" => "English".into(),
        "muted" => {
            if zh {
                "静音".into()
            } else {
                "Muted".into()
            }
        }
        "about_text" => {
            if zh {
                "Audio Switcher — 托盘音频切换工具\n纯 Rust 托盘工具\n\n右键菜单切换设备，中键静音，悬停滚轮调音量。".into()
            } else {
                "Audio Switcher — Tray audio switcher\nPure Rust tray tool\n\nRight-click to switch device, middle-click to mute, hover+wheel to adjust volume.".into()
            }
        }
        _ => key.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Lang;

    #[test]
    fn i18n_zh_en() {
        assert_eq!(tr("mute", Lang::Zh), "全局静音");
        assert_eq!(tr("mute", Lang::En), "Mute");
        assert_eq!(tr("volume_limit", Lang::Zh), "音量上限");
        assert_eq!(tr("experimental", Lang::En), "Experimental");
    }
}

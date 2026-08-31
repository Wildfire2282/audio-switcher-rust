#![windows_subsystem = "windows"]
use audio_switcher_rust::platform::{ComGuard, SingleInstanceGuard};

fn main() {
    let Some(_guard) = SingleInstanceGuard::new("audio-switcher-rust-single-instance-v1") else {
        return;
    };

    let Some(com) = ComGuard::init() else {
        return;
    };

    let app = audio_switcher_rust::app::App::new(com);
    app.run();
}

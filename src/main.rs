//! Binary entry — minimal `main` per `proj-lib-main-split`.
#![windows_subsystem = "windows"]
use audio_switcher_rust::{ComGuard, SingleInstanceGuard};

fn main() {
    let Some(_guard) = SingleInstanceGuard::new("audio-switcher-rust-single-instance-v1") else {
        // Already running — exit 0; no UI needed (second instance).
        std::process::exit(0);
    };

    let Some(com) = ComGuard::init() else {
        // COM init failed — ComGuard::init already showed MessageBox; exit 1 so launcher sees error.
        std::process::exit(1);
    };

    let app = audio_switcher_rust::app::App::new(com);
    app.run();
}

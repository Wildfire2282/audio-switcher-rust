#![windows_subsystem = "windows"]

mod app;
mod audio;
mod config;
mod platform;
mod system;
mod ui;

// legacy re-export for callers using `crate::tray::*`
mod tray;

use platform::ComGuard;

fn main() {
    let _guard = match system::SingleInstanceGuard::new("audio-switcher-rust-single-instance-v1") {
        Some(g) => g,
        None => return,
    };

    let com = match ComGuard::init() {
        Some(c) => c,
        None => return,
    };

    let app = app::App::new(com);
    app.run();
}

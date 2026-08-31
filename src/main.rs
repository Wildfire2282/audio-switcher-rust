#![windows_subsystem = "windows"]

mod app;
mod audio;
mod config;
mod platform;
mod ui;

use platform::{ComGuard, SingleInstanceGuard};

fn main() {
    let _guard = match SingleInstanceGuard::new("audio-switcher-rust-single-instance-v1") {
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

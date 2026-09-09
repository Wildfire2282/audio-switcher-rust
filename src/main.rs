//! Binary entry: crash reporting → guards → `App::run`.
#![windows_subsystem = "windows"]
// Baseline duplicates: same allow as lib.rs (separate crate root).
#![allow(clippy::multiple_crate_versions)]
use audio_switcher::{ComGuard, SingleInstanceGuard, init_crash_reporting, show_critical};

fn main() {
    init_crash_reporting();
    // Second instance exits silently (0); creation failure dialogs + exits (1).
    let _guard = match SingleInstanceGuard::acquire(&audio_switcher::single_instance_id()) {
        Ok(Some(guard)) => guard,
        Ok(None) => std::process::exit(0),
        Err(e) => fatal("Another copy may be starting.", e),
    };
    let com = match ComGuard::init() {
        Ok(com) => com,
        Err(e) => fatal("COM startup failed, exiting.", e),
    };
    match audio_switcher::app::App::new(com) {
        Ok(app) => app.run(),
        Err(e) => fatal("Tray startup failed, exiting.", e),
    }
}

/// Show a critical dialog with the full error chain (`{:#}` renders sources)
/// and exit(1). Guards-only helper; no business logic.
fn fatal(message: &str, detail: impl std::fmt::Display) -> ! {
    show_critical(&format!("{message}\n\nDetails: {detail:#}"));
    std::process::exit(1);
}

//! Crash reporting: file sink + panic hook, installed once from `main`.
//!
//! Libraries never install a subscriber; this helper only exists so the
//! binary entry stays at guards → `App::run`. The install site is `main`.

/// Install the `%LOCALAPPDATA%\<tool>\logs\` file sink and the
/// show-dialog-and-exit panic hook. Idempotent best effort: when the log
/// file cannot be opened, diagnostics still reach the dialog on panic.
pub fn init() {
    let log_path = log_file_path();

    let file = log_path
        .parent()
        .map(std::fs::create_dir_all)
        .and(std::fs::File::create(&log_path).ok());
    if let Some(file) = file {
        // File sink only: `windows_subsystem = "windows"` detaches stdio, so
        // a console layer would be invisible; the file is the record.
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .with_writer(std::sync::Mutex::new(file))
            .finish();
        let _ = tracing::subscriber::set_global_default(subscriber);
    }

    std::panic::set_hook(Box::new(|info| {
        let msg = format!(
            "{} hit an unexpected error and must exit.\n\n{info}",
            crate::TOOL_DISPLAY_NAME
        );
        tracing::error!("panic: {info}");
        crate::platform::dialog::show_critical(&msg);
        std::process::exit(1);
    }));

    tracing::debug!("logging to {}", log_file_path().display());
}

/// Daily log file: day-indexed name gives rotation without a date library
/// (SystemTime days since epoch; a wall-clock jump only renames the file).
fn log_file_path() -> std::path::PathBuf {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0);
    log_dir().join(format!("{}-{days}.log", crate::TOOL_ID))
}

fn log_dir() -> std::path::PathBuf {
    let base = std::env::var("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    base.join(crate::TOOL_ID).join("logs")
}

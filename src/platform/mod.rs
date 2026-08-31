//! Windows platform abstractions — COM, hooks, shell and autostart.

pub mod autostart;
pub mod com;
pub mod dialog;
pub mod hook;
pub mod instance;
pub mod shell;

pub(crate) use autostart::{is_autostart_enabled, set_autostart};
pub use com::ComGuard;
pub(crate) use dialog::{prompt_custom_limit, show_autostart_error};
pub use instance::SingleInstanceGuard;

pub(crate) use hook::{cursor_over_tray, peek_pending, take_delta, take_pending};

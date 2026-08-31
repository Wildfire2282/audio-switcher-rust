pub mod autostart;
pub mod com;
pub mod dialog;
pub mod hook;
pub mod instance;
pub mod shell;

pub use autostart::{is_autostart_enabled, set_autostart};
pub use com::ComGuard;
pub use dialog::{prompt_custom_limit, show_autostart_error};
pub use instance::SingleInstanceGuard;

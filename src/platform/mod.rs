pub mod com;
pub mod dialog;
pub mod hook;
pub mod instance;
pub mod autostart;
pub mod shell;

pub use com::ComGuard;
#[allow(unused_imports)]
pub use dialog::{prompt_custom_limit, show_autostart_error, show_error_invalid_custom};
#[cfg(windows)]
#[allow(unused_imports)]
pub use dialog::show_msgbox;
#[allow(unused_imports)]
pub use hook::WheelHook;
#[allow(unused_imports)]
pub use instance::SingleInstanceGuard;
#[allow(unused_imports)]
pub use autostart::{get_exe_path, is_autostart_enabled, set_autostart};

//! Windows platform abstractions: guards, shell, dialogs, pump, autostart.
//!
//! `platform/*` modules are mutually unreferenced except for foundational
//! use: `dialog` (message boxes) and `shell` may be called from `app`/`ui`,
//! and `logging`/`shell` format `autostart` errors. Those edges are noted on
//! the callee side per the module contract.

pub mod autostart;
pub mod com;
pub mod dialog;
pub mod hook;
pub mod instance;
pub mod locale;
pub mod logging;
pub mod pump;
pub mod shell;

pub use autostart::{AutostartState, autostart_state, set_autostart};
pub use com::{ComError, ComGuard};
pub use instance::{InstanceError, SingleInstanceGuard};

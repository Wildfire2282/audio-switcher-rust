//! Facade for backward compatibility. New code uses `crate::platform::*`.
#![allow(unused_imports)]
pub use crate::platform::instance::SingleInstanceGuard;
pub use crate::platform::autostart::{get_exe_path, is_autostart_enabled, set_autostart};
pub use crate::platform::dialog::show_autostart_error;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn single_instance_name() {
        let g = SingleInstanceGuard::new("audio-switcher-rust-test-single-instance-unique-12345-facade");
        assert!(g.is_some());
    }
}

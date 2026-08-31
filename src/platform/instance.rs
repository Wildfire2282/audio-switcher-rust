//! Single-instance guard — prevents multiple tray processes.

use single_instance::SingleInstance;

/// RAII guard ensuring only one instance runs with the given `name`.
pub struct SingleInstanceGuard {
    _instance: SingleInstance,
}

impl SingleInstanceGuard {
    /// Try to acquire the single-instance lock.
    ///
    /// Returns `None` if another instance already holds the lock or if the
    /// underlying OS primitive cannot be created.
    #[must_use]
    pub fn new(name: &str) -> Option<Self> {
        let instance = SingleInstance::new(name).ok()?;
        if !instance.is_single() {
            return None;
        }
        Some(Self { _instance: instance })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_instance_name() {
        let g = SingleInstanceGuard::new("audio-switcher-rust-test-single-instance-unique-12345");
        assert!(g.is_some());
    }
}

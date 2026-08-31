use single_instance::SingleInstance;

pub struct SingleInstanceGuard {
    _instance: SingleInstance,
}

impl SingleInstanceGuard {
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

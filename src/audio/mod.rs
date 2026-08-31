use crate::config::AppConfig;

#[derive(Debug, Clone, PartialEq)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub enum AudioError {
    Com(String),
    Failed(String),
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioError::Com(s) => write!(f, "COM error: {}", s),
            AudioError::Failed(s) => write!(f, "failed: {}", s),
        }
    }
}
impl std::error::Error for AudioError {}

pub trait AudioBackend {
    fn enumerate_devices(&mut self) -> Result<Vec<AudioDevice>, AudioError>;
    fn get_default_device(&self) -> Option<AudioDevice>;
    fn set_default_device(&mut self, id: &str) -> Result<(), AudioError>;
    fn get_volume(&self) -> Result<u32, AudioError>;
    fn set_volume(&mut self, volume: u32) -> Result<(), AudioError>;
    fn get_mute(&self) -> Result<bool, AudioError>;
    fn set_mute(&mut self, mute: bool) -> Result<(), AudioError>;
    fn clamp_volume_if_needed(&mut self, cfg: &AppConfig) -> Result<(), AudioError>;
}

/// 快照：一次性获取枚举/默认/音量/静音，避免多次 CoCreateInstance
#[derive(Debug, Clone)]
pub struct AudioSnapshot {
    pub devices: Vec<AudioDevice>,
    pub default_device: Option<AudioDevice>,
    pub volume: u32,
    pub mute: bool,
}

impl Default for AudioSnapshot {
    fn default() -> Self {
        Self { devices: Vec::new(), default_device: None, volume: 50, mute: false }
    }
}

#[cfg(test)]
pub mod mock;
#[cfg(test)]
pub use mock::MockBackend;

pub mod real;
pub use real::{take_device_changed, RealBackend};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_enumerate_cache() {
        let devs = vec![AudioDevice { id: "a".into(), name: "Speaker".into() }];
        let mut m = MockBackend::new(devs.clone(), Some("a".into()));
        let first = m.enumerate_devices().unwrap();
        assert_eq!(first.len(), 1);
        let count_before = m.enumerate_count;
        let second = m.enumerate_devices().unwrap();
        assert_eq!(second.len(), 1);
        assert_eq!(m.enumerate_count, count_before, "should hit cache within 800ms");
        std::thread::sleep(std::time::Duration::from_millis(850));
        let _third = m.enumerate_devices().unwrap();
        assert_eq!(m.enumerate_count, count_before + 1);
    }

    #[test]
    fn mock_empty_keeps_current() {
        let devs = vec![AudioDevice { id: "a".into(), name: "Sp".into() }];
        let mut m = MockBackend::new(devs.clone(), Some("a".into()));
        let _ = m.enumerate_devices().unwrap();
        m.devices = vec![];
        std::thread::sleep(std::time::Duration::from_millis(850));
        let after = m.enumerate_devices().unwrap();
        assert_eq!(after.len(), 1);
    }

    #[test]
    fn clamp_via_backend() {
        let cfg = AppConfig { volume_limit: 25, volume_limit_enabled: true, ..Default::default() };
        let mut m = MockBackend::new(vec![], None);
        m.volume = 80;
        m.clamp_volume_if_needed(&cfg).unwrap();
        assert_eq!(m.volume, 25);
    }

    #[test]
    fn set_default_device_mock() {
        let devs = vec![
            AudioDevice { id: "a".into(), name: "A".into() },
            AudioDevice { id: "b".into(), name: "B".into() },
        ];
        let mut m = MockBackend::new(devs, Some("a".into()));
        m.set_default_device("b").unwrap();
        assert_eq!(m.default_id.as_deref(), Some("b"));
        assert!(m.set_default_device("c").is_err());
    }

    #[test]
    #[ignore]
    fn integration_real_mock() {
        let devs = vec![AudioDevice { id: "x".into(), name: "X".into() }];
        let mut backend: Box<dyn AudioBackend> = Box::new(MockBackend::new(devs, None));
        let list = backend.enumerate_devices().unwrap();
        assert_eq!(list.len(), 1);
    }
}

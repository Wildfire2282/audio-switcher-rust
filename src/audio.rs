use std::time::{Duration, Instant};

use crate::config::{clamp_volume, AppConfig};

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

// ------------------------------------------------------------
// MockBackend for tests
// ------------------------------------------------------------
#[cfg(test)]
#[derive(Debug, Clone)]
pub struct MockBackend {
    pub devices: Vec<AudioDevice>,
    pub default_id: Option<String>,
    pub volume: u32,
    pub mute: bool,
    pub fail_next: bool,
    pub enumerate_count: usize,
    cached: Option<Vec<AudioDevice>>,
    cache_time: Option<Instant>,
}

#[cfg(test)]
impl MockBackend {
    pub fn new(devices: Vec<AudioDevice>, default_id: Option<String>) -> Self {
        Self {
            devices,
            default_id,
            volume: 50,
            mute: false,
            fail_next: false,
            enumerate_count: 0,
            cached: None,
            cache_time: None,
        }
    }

    fn maybe_fail(&mut self) -> Option<AudioError> {
        if self.fail_next {
            self.fail_next = false;
            return Some(AudioError::Failed("mock failure".into()));
        }
        None
    }

    pub fn set_volume_impl(&mut self, volume: u32) -> Result<(), AudioError> {
        if let Some(e) = self.maybe_fail() {
            return Err(e);
        }
        self.volume = volume.min(100);
        Ok(())
    }
}

#[cfg(test)]
impl AudioBackend for MockBackend {
    fn enumerate_devices(&mut self) -> Result<Vec<AudioDevice>, AudioError> {
        if let Some(cached) = &self.cached {
            if let Some(t) = &self.cache_time {
                if t.elapsed() < Duration::from_millis(800) {
                    return Ok(cached.clone());
                }
            }
        }
        if self.devices.is_empty() {
            if let Some(c) = &self.cached {
                return Ok(c.clone());
            }
            return Ok(vec![]);
        }
        self.enumerate_count += 1;
        let v = self.devices.clone();
        self.cached = Some(v.clone());
        self.cache_time = Some(Instant::now());
        Ok(v)
    }

    fn get_default_device(&self) -> Option<AudioDevice> {
        let id = self.default_id.as_ref()?;
        self.devices.iter().find(|d| &d.id == id).cloned()
    }

    fn set_default_device(&mut self, id: &str) -> Result<(), AudioError> {
        if let Some(e) = self.maybe_fail() {
            return Err(e);
        }
        if self.devices.iter().any(|d| d.id == id) {
            self.default_id = Some(id.to_string());
            Ok(())
        } else {
            Err(AudioError::Failed(format!("not found: {}", id)))
        }
    }

    fn get_volume(&self) -> Result<u32, AudioError> {
        Ok(self.volume)
    }

    fn set_volume(&mut self, volume: u32) -> Result<(), AudioError> {
        self.set_volume_impl(volume)
    }

    fn get_mute(&self) -> Result<bool, AudioError> {
        Ok(self.mute)
    }

    fn set_mute(&mut self, mute: bool) -> Result<(), AudioError> {
        if let Some(e) = self.maybe_fail() {
            return Err(e);
        }
        self.mute = mute;
        Ok(())
    }

    fn clamp_volume_if_needed(&mut self, cfg: &AppConfig) -> Result<(), AudioError> {
        let clamped = clamp_volume(self.volume, cfg);
        if clamped != self.volume {
            self.volume = clamped;
        }
        Ok(())
    }
}

// ------------------------------------------------------------
// RealBackend (Windows only)
// ------------------------------------------------------------
#[cfg(windows)]
pub mod real {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
    use windows::core::{Interface, GUID, HRESULT, PCWSTR};
    use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        eMultimedia, eRender, IMMDevice, IMMDeviceCollection, IMMDeviceEnumerator,
        IMMNotificationClient, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
    };
    use windows::Win32::System::Com::StructuredStorage::PropVariantClear;
    use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL, STGM_READ};
    use windows::Win32::System::Variant::VT_LPWSTR;
    type SetDefaultEndpointFn =
        unsafe extern "system" fn(*mut std::ffi::c_void, PCWSTR, i32) -> HRESULT;

    unsafe fn set_default_endpoint_raw(device_id: &str, role: i32) -> windows::core::Result<()> {
        use windows::core::IUnknown;
        // 多 CLSID/虚表偏移回退：不同 Windows 11 版本上
        // 仅用单一 GUID/偏移会导致 0x80040154 或误调 SetEndpointVisibility
        // 而呈现“不报错但不切换”。
        // 以 audioswitch/IPolicyConfig.h 为准：
        //   IPolicyConfig::SetDefaultEndpoint @ vtbl[13]
        //   IPolicyConfigVista::SetDefaultEndpoint @ vtbl[12]
        // 每个 CLSID 绑定其规范 IID 与主偏移，仅主偏移 S_OK 才算成功，
        // 避免对 Vista 客户端先试 13 误调 SetEndpointVisibility 造成的假成功。
        const IID_IPOLICYCONFIG: GUID = GUID::from_u128(0xf8679f50_850a_41cf_9c72_430f290290c8);
        const IID_IPOLICYCONFIG_VISTA: GUID =
            GUID::from_u128(0x568b9108_44bf_40b4_9006_86afe5b5a620);
        const CANDIDATES: &[(GUID, GUID, usize)] = &[
            (
                GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9),
                IID_IPOLICYCONFIG,
                13,
            ),
            (
                GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2ea9),
                IID_IPOLICYCONFIG,
                13,
            ),
            (
                GUID::from_u128(0x294935ce_f637_4e7c_a41b_ab255460b862),
                IID_IPOLICYCONFIG_VISTA,
                12,
            ),
            (
                GUID::from_u128(0x294935ce_f588_4bd5_9f8c_bab13166b487),
                IID_IPOLICYCONFIG_VISTA,
                12,
            ),
        ];
        type QiFn = unsafe extern "system" fn(
            *mut std::ffi::c_void,
            *const GUID,
            *mut *mut std::ffi::c_void,
        ) -> HRESULT;
        type ReleaseFn = unsafe extern "system" fn(*mut std::ffi::c_void) -> u32;
        let wide: Vec<u16> = device_id.encode_utf16().chain(std::iter::once(0)).collect();
        let mut last_err: Option<windows::core::Error> = None;
        for &(clsid, iid, primary_off) in CANDIDATES {
            let instance: windows::core::Result<IUnknown> =
                CoCreateInstance(&clsid, None, CLSCTX_ALL);
            let Ok(unk) = instance else {
                if let Err(e) = instance {
                    last_err = Some(e);
                }
                continue;
            };
            // 1) 优先 QI 到规范 IID 后在该接口指针上调用主偏移
            let raw_unk = unk.as_raw();
            let vtbl_unk = unsafe { *(raw_unk as *mut *mut *mut std::ffi::c_void) };
            if !vtbl_unk.is_null() {
                let qi: QiFn = unsafe { std::mem::transmute(*vtbl_unk) };
                let mut iface: *mut std::ffi::c_void = std::ptr::null_mut();
                let hr_qi = unsafe { qi(raw_unk, &iid as *const GUID, &mut iface) };
                if hr_qi.is_ok() && !iface.is_null() {
                    let vtbl_iface = unsafe { *(iface as *mut *mut *mut std::ffi::c_void) };
                    if !vtbl_iface.is_null() {
                        let func: SetDefaultEndpointFn =
                            unsafe { std::mem::transmute(*vtbl_iface.add(primary_off)) };
                        let hr = unsafe { func(iface, PCWSTR(wide.as_ptr()), role) };
                        // Release iface
                        let rel: ReleaseFn =
                            unsafe { std::mem::transmute(*vtbl_iface.add(2)) };
                        unsafe { rel(iface) };
                        if hr.is_ok() {
                            return Ok(());
                        }
                        last_err = Some(windows::core::Error::from(hr));
                        // Vista 上若主偏移失败，不再对该 CLSID 试另一偏移，
                        // 避免误调另一个方法产生假 S_OK；直接试下一 CLSID。
                        continue;
                    }
                    // QI 成功但 vtbl 空，兜底 Release
                    let vtbl_iface = unsafe { *(iface as *mut *mut *mut std::ffi::c_void) };
                    if !vtbl_iface.is_null() {
                        let rel: ReleaseFn =
                            unsafe { std::mem::transmute(*vtbl_iface.add(2)) };
                        unsafe { rel(iface) };
                    }
                }
            }
            // 2) QI 失败或环境不支持 QI，回退到直接在 IUnknown 裸指针上按主偏移调用
            //    （多数系统上具体类直接实现接口，裸调同样有效）
            let vtbl = unsafe { *(raw_unk as *mut *mut *mut std::ffi::c_void) };
            if vtbl.is_null() {
                last_err = Some(windows::core::Error::from_hresult(windows::core::HRESULT(
                    0x80004005u32 as i32,
                )));
                continue;
            }
            let func: SetDefaultEndpointFn =
                unsafe { std::mem::transmute(*vtbl.add(primary_off)) };
            let hr = unsafe { func(raw_unk, PCWSTR(wide.as_ptr()), role) };
            if hr.is_ok() {
                return Ok(());
            }
            last_err = Some(windows::core::Error::from(hr));
        }
        Err(last_err.unwrap_or(windows::core::Error::from_hresult(
            windows::core::HRESULT(0x80040154u32 as i32),
        )))
    }

    static DEVICE_CHANGED: AtomicBool = AtomicBool::new(false);

    static SUPPRESS_NOTIFY: AtomicBool = AtomicBool::new(false);

    pub fn take_device_changed() -> bool {
        DEVICE_CHANGED.swap(false, AtomicOrdering::Relaxed)
    }

    #[windows_core::implement(windows::Win32::Media::Audio::IMMNotificationClient)]
    struct Notifier;

    impl windows::Win32::Media::Audio::IMMNotificationClient_Impl for Notifier_Impl {
        fn OnDeviceStateChanged(
            &self,
            _device_id: &PCWSTR,
            _new_state: windows::Win32::Media::Audio::DEVICE_STATE,
        ) -> windows::core::Result<()> {
            DEVICE_CHANGED.store(true, AtomicOrdering::Relaxed);
            Ok(())
        }
        fn OnDeviceAdded(&self, _device_id: &PCWSTR) -> windows::core::Result<()> {
            DEVICE_CHANGED.store(true, AtomicOrdering::Relaxed);
            Ok(())
        }
        fn OnDeviceRemoved(&self, _device_id: &PCWSTR) -> windows::core::Result<()> {
            DEVICE_CHANGED.store(true, AtomicOrdering::Relaxed);
            Ok(())
        }
        fn OnDefaultDeviceChanged(
            &self,
            _flow: windows::Win32::Media::Audio::EDataFlow,
            _role: windows::Win32::Media::Audio::ERole,
            _device_id: &PCWSTR,
        ) -> windows::core::Result<()> {
            DEVICE_CHANGED.store(true, AtomicOrdering::Relaxed);
            Ok(())
        }
        fn OnPropertyValueChanged(
            &self,
            _device_id: &PCWSTR,
            _key: &windows::Win32::Foundation::PROPERTYKEY,
        ) -> windows::core::Result<()> {
            if SUPPRESS_NOTIFY.load(AtomicOrdering::Relaxed) {
                return Ok(());
            }
            DEVICE_CHANGED.store(true, AtomicOrdering::Relaxed);
            Ok(())
        }
    }

    fn register_notification_client() {
        unsafe {
            if let Ok(enumerator) =
                CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL)
            {
                let notifier: IMMNotificationClient = Notifier.into();
                let _ = enumerator.RegisterEndpointNotificationCallback(&notifier);
                // leak to keep alive for process lifetime
                std::mem::forget(notifier);
            }
        }
    }

    pub struct RealBackend {
        cached: Option<Vec<AudioDevice>>,
        cache_time: Option<Instant>,
        // 复用 enumerator / endpoint 避免重复 CoCreateInstance (启动阶段批量查询)
        cached_enumerator: Option<windows::Win32::Media::Audio::IMMDeviceEnumerator>,
    }

    impl RealBackend {
        pub fn new() -> Self {
            let s = Self {
                cached: None,
                cache_time: None,
                cached_enumerator: None,
            };
            // register once per process
            static REGISTERED: AtomicBool = AtomicBool::new(false);
            if !REGISTERED.swap(true, AtomicOrdering::SeqCst) {
                register_notification_client();
            }
            s
        }

        pub fn clear_cache(&mut self) {
            self.cached = None;
            self.cache_time = None;
            // enumerator 保持可用，无需清除；仅设备列表缓存失效
        }

        /// 获取或创建缓存的 IMMDeviceEnumerator，减少 CoCreateInstance 次数
        fn enumerator_mut(&mut self) -> windows::core::Result<windows::Win32::Media::Audio::IMMDeviceEnumerator> {
            if let Some(e) = &self.cached_enumerator {
                return Ok(e.clone());
            }
            let e = Self::get_enumerator()?;
            self.cached_enumerator = Some(e.clone());
            Ok(e)
        }

        #[allow(dead_code)]
        /// 批量获取启动所需状态：设备列表 + 默认设备 + 音量 + 静音，共享同一个 enumerator/endpoint
        pub fn fetch_snapshot(&mut self) -> AudioSnapshot {
            let mut snap = AudioSnapshot::default();
            // 尽量用缓存的 enumerator
            let enumerator = match self.enumerator_mut() {
                Ok(e) => e,
                Err(_) => return snap,
            };
            // devices (带 3000ms 缓存，通知已清除缓存，延长命中)
            if let Ok(devs) = self.enumerate_devices_inner(&enumerator) {
                snap.devices = devs;
            }
            // default + volume/mute 用同一个 endpoint
            unsafe {
                if let Ok(dev) = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia) {
                    if let Ok(id) = Self::device_id(&dev) {
                        let name = Self::device_friendly_name(&dev);
                        snap.default_device = Some(AudioDevice { id, name });
                    }
                    if let Ok(vol) = dev.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) {
                        if let Ok(scalar) = vol.GetMasterVolumeLevelScalar() {
                            snap.volume = (scalar * 100.0).round() as u32;
                        }
                        if let Ok(m) = vol.GetMute() {
                            snap.mute = m.as_bool();
                        }
                    }
                }
            }
            snap
        }

        /// 合并 clamp 的快照：内部一次性处理限幅，避免二次 get_volume_and_mute
        pub fn fetch_snapshot_clamped(&mut self, cfg: &AppConfig) -> AudioSnapshot {
            let mut snap = AudioSnapshot::default();
            let enumerator = match self.enumerator_mut() {
                Ok(e) => e,
                Err(_) => return snap,
            };
            if let Ok(devs) = self.enumerate_devices_inner(&enumerator) {
                snap.devices = devs;
            }
            unsafe {
                if let Ok(dev) = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia) {
                    if let Ok(id) = Self::device_id(&dev) {
                        let name = Self::device_friendly_name(&dev);
                        snap.default_device = Some(AudioDevice { id, name });
                    }
                    if let Ok(vol) = dev.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) {
                        if let Ok(scalar) = vol.GetMasterVolumeLevelScalar() {
                            snap.volume = (scalar * 100.0).round() as u32;
                        }
                        if let Ok(m) = vol.GetMute() {
                            snap.mute = m.as_bool();
                        }
                        // 一次性限幅，避免外部二次 get_volume_and_mute
                        if cfg.volume_limit_enabled {
                            let clamped = clamp_volume(snap.volume, cfg);
                            if clamped != snap.volume {
                                SUPPRESS_NOTIFY.store(true, AtomicOrdering::Relaxed);
                                let v = clamped.min(100) as f32 / 100.0;
                                let _ = vol.SetMasterVolumeLevelScalar(v, std::ptr::null());
                                SUPPRESS_NOTIFY.store(false, AtomicOrdering::Relaxed);
                                snap.volume = clamped;
                            }
                        }
                    }
                }
            }
            snap
        }

        /// 一次 endpoint 激活同时获取音量+静音，减少一次 CoCreateInstance+Activate
        /// 复用 cached_enumerator 而非每次新建
        pub fn get_volume_and_mute(&self) -> Result<(u32, bool), AudioError> {
            unsafe {
                let enumerator = if let Some(e) = &self.cached_enumerator {
                    e.clone()
                } else {
                    Self::get_enumerator().map_err(|e| AudioError::Com(e.to_string()))?
                };
                let dev = enumerator
                    .GetDefaultAudioEndpoint(eRender, eMultimedia)
                    .map_err(|e| AudioError::Com(e.to_string()))?;
                let vol: IAudioEndpointVolume = dev
                    .Activate(CLSCTX_ALL, None)
                    .map_err(|e| AudioError::Com(e.to_string()))?;
                let scalar = vol
                    .GetMasterVolumeLevelScalar()
                    .map_err(|e| AudioError::Com(e.to_string()))?;
                let m = vol.GetMute().map_err(|e| AudioError::Com(e.to_string()))?;
                Ok(((scalar * 100.0).round() as u32, m.as_bool()))
            }
        }

        fn enumerate_devices_inner(
            &mut self,
            enumerator: &windows::Win32::Media::Audio::IMMDeviceEnumerator,
        ) -> Result<Vec<AudioDevice>, AudioError> {
            if take_device_changed() {
                self.clear_cache();
            }
            if let Some(cached) = &self.cached {
                if let Some(t) = &self.cache_time {
                    if t.elapsed() < Duration::from_millis(3000) {
                        return Ok(cached.clone());
                    }
                }
            }
            unsafe {
                let collection: IMMDeviceCollection = enumerator
                    .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)
                    .map_err(|e| AudioError::Com(e.to_string()))?;
                let count = collection
                    .GetCount()
                    .map_err(|e| AudioError::Com(e.to_string()))?;
                let mut devices = Vec::new();
                for i in 0..count {
                    if let Ok(dev) = collection.Item(i) {
                        if let Ok(id) = Self::device_id(&dev) {
                            let name = Self::device_friendly_name(&dev);
                            devices.push(AudioDevice { id, name });
                        }
                    }
                }
                if devices.is_empty() {
                    if let Some(c) = &self.cached {
                        return Ok(c.clone());
                    }
                    return Ok(vec![]);
                }
                self.cached = Some(devices.clone());
                self.cache_time = Some(Instant::now());
                Ok(devices)
            }
        }

        pub fn poll_device_changed(&mut self) -> bool {
            if take_device_changed() {
                self.clear_cache();
                return true;
            }
            false
        }

        fn get_enumerator() -> windows::core::Result<IMMDeviceEnumerator> {
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
        }

        fn device_id(device: &IMMDevice) -> windows::core::Result<String> {
            unsafe {
                let pw = device.GetId()?;
                let s = pw.to_string().unwrap_or_default();
                CoTaskMemFree(Some(pw.0 as *const std::ffi::c_void));
                Ok(s)
            }
        }

        fn device_friendly_name(device: &IMMDevice) -> String {
            unsafe {
                if let Ok(store) = device.OpenPropertyStore(STGM_READ) {
                    if let Ok(mut pv) = store.GetValue(&PKEY_Device_FriendlyName) {
                        let vt = pv.Anonymous.Anonymous.vt;
                        let s = if vt == VT_LPWSTR {
                            let pw = pv.Anonymous.Anonymous.Anonymous.pwszVal;
                            if !pw.0.is_null() {
                                pw.to_string().unwrap_or_default()
                            } else {
                                String::new()
                            }
                        } else {
                            String::new()
                        };
                        let _ = PropVariantClear(&mut pv as *mut _);
                        if !s.is_empty() {
                            return s.chars().take(80).collect();
                        }
                    }
                }
                // Fallback to ID short
                if let Ok(id) = Self::device_id(device) {
                    let short = id.split('\\').next_back().unwrap_or(&id);
                    let truncated: String = short.chars().take(40).collect();
                    if !truncated.is_empty() {
                        return truncated;
                    }
                    return id.chars().take(40).collect();
                }
                "Unknown".to_string()
            }
        }

        fn show_msgbox(msg: &str) {
            crate::platform::dialog::show_msgbox(msg);
        }
    }

    impl AudioBackend for RealBackend {
        fn enumerate_devices(&mut self) -> Result<Vec<AudioDevice>, AudioError> {
            let enumerator = self
                .enumerator_mut()
                .map_err(|e| AudioError::Com(e.to_string()))?;
            self.enumerate_devices_inner(&enumerator)
        }

        fn get_default_device(&self) -> Option<AudioDevice> {
            unsafe {
                let enumerator = if let Some(e) = &self.cached_enumerator {
                    e.clone()
                } else {
                    Self::get_enumerator().ok()?
                };
                let dev = enumerator
                    .GetDefaultAudioEndpoint(eRender, eMultimedia)
                    .ok()?;
                let id = Self::device_id(&dev).ok()?;
                let name = Self::device_friendly_name(&dev);
                Some(AudioDevice { id, name })
            }
        }

        fn set_default_device(&mut self, id: &str) -> Result<(), AudioError> {
            unsafe {
                let hr = set_default_endpoint_raw(id, eMultimedia.0);
                if let Err(e) = hr {
                    return Err(AudioError::Failed(e.to_string()));
                }
                let _ = set_default_endpoint_raw(id, 0);
                let _ = set_default_endpoint_raw(id, 2);
                self.clear_cache();
                Ok(())
            }
        }

        fn get_volume(&self) -> Result<u32, AudioError> {
            self.get_volume_and_mute().map(|(v, _)| v)
        }

        fn set_volume(&mut self, volume: u32) -> Result<(), AudioError> {
            let v = volume.min(100) as f32 / 100.0;
            unsafe {
                let enumerator = self
                    .enumerator_mut()
                    .map_err(|e| AudioError::Com(e.to_string()))?;
                let dev = enumerator
                    .GetDefaultAudioEndpoint(eRender, eMultimedia)
                    .map_err(|e| AudioError::Com(e.to_string()))?;
                let vol: IAudioEndpointVolume = dev
                    .Activate(CLSCTX_ALL, None)
                    .map_err(|e| AudioError::Com(e.to_string()))?;
                let mut last_err: Option<windows::core::Error> = None;
                for _ in 0..2 {
                    match vol.SetMasterVolumeLevelScalar(v, std::ptr::null()) {
                        Ok(()) => return Ok(()),
                        Err(e) => last_err = Some(e),
                    }
                }
                if let Some(e) = last_err {
                    Self::show_msgbox(&format!("设置音量失败: {}", e));
                    return Err(AudioError::Failed(e.to_string()));
                }
                Ok(())
            }
        }

        fn get_mute(&self) -> Result<bool, AudioError> {
            self.get_volume_and_mute().map(|(_, m)| m)
        }

        fn set_mute(&mut self, mute: bool) -> Result<(), AudioError> {
            unsafe {
                let enumerator = self
                    .enumerator_mut()
                    .map_err(|e| AudioError::Com(e.to_string()))?;
                let dev = enumerator
                    .GetDefaultAudioEndpoint(eRender, eMultimedia)
                    .map_err(|e| AudioError::Com(e.to_string()))?;
                let vol: IAudioEndpointVolume = dev
                    .Activate(CLSCTX_ALL, None)
                    .map_err(|e| AudioError::Com(e.to_string()))?;
                for _ in 0..2 {
                    match vol.SetMute(mute, std::ptr::null()) {
                        Ok(()) => return Ok(()),
                        Err(_) => continue,
                    }
                }
                Self::show_msgbox("切换静音失败");
                Err(AudioError::Failed("set mute failed".into()))
            }
        }

        fn clamp_volume_if_needed(&mut self, cfg: &AppConfig) -> Result<(), AudioError> {
            if !cfg.volume_limit_enabled {
                return Ok(());
            }
            // 复用一次激活同时取音量，减少 CoCreateInstance
            let (vol, _) = self.get_volume_and_mute()?;
            let clamped = clamp_volume(vol, cfg);
            if clamped != vol {
                SUPPRESS_NOTIFY.store(true, AtomicOrdering::Relaxed);
                let r = self.set_volume(clamped);
                SUPPRESS_NOTIFY.store(false, AtomicOrdering::Relaxed);
                return r;
            }
            Ok(())
        }
    }
}

#[cfg(not(windows))]
pub mod real {
    use super::*;
    use windows::Win32::Foundation::HANDLE;
    pub struct RealBackend {
        cached: Option<Vec<AudioDevice>>,
        cache_time: Option<Instant>,
    }
    impl RealBackend {
        pub fn new() -> Self {
            Self {
                cached: None,
                cache_time: None,
            }
        }
        pub fn clear_cache(&mut self) {
            self.cached = None;
            self.cache_time = None;
        }
        pub fn poll_device_changed(&mut self) -> bool {
            false
        }
        pub fn fetch_snapshot(&mut self) -> AudioSnapshot {
            AudioSnapshot::default()
        }
        pub fn fetch_snapshot_clamped(&mut self, _cfg: &AppConfig) -> AudioSnapshot {
            AudioSnapshot::default()
        }
        pub fn get_volume_and_mute(&self) -> Result<(u32, bool), AudioError> {
            Ok((50, false))
        }
    }
    impl AudioBackend for RealBackend {
        fn enumerate_devices(&mut self) -> Result<Vec<AudioDevice>, AudioError> {
            Ok(vec![])
        }
        fn get_default_device(&self) -> Option<AudioDevice> {
            None
        }
        fn set_default_device(&mut self, _id: &str) -> Result<(), AudioError> {
            Ok(())
        }
        fn get_volume(&self) -> Result<u32, AudioError> {
            Ok(50)
        }
        fn set_volume(&mut self, _volume: u32) -> Result<(), AudioError> {
            Ok(())
        }
        fn get_mute(&self) -> Result<bool, AudioError> {
            Ok(false)
        }
        fn set_mute(&mut self, _mute: bool) -> Result<(), AudioError> {
            Ok(())
        }
        fn clamp_volume_if_needed(&mut self, _cfg: &AppConfig) -> Result<(), AudioError> {
            Ok(())
        }
    }
    pub fn take_device_changed() -> bool {
        false
    }
    pub fn device_event_handle() -> windows::Win32::Foundation::HANDLE {
        windows::Win32::Foundation::HANDLE::default()
    }
}

pub use real::{take_device_changed, RealBackend};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_enumerate_cache() {
        let devs = vec![AudioDevice {
            id: "a".into(),
            name: "Speaker".into(),
        }];
        let mut m = MockBackend::new(devs.clone(), Some("a".into()));
        let first = m.enumerate_devices().unwrap();
        assert_eq!(first.len(), 1);
        let count_before = m.enumerate_count;
        let second = m.enumerate_devices().unwrap();
        assert_eq!(second.len(), 1);
        assert_eq!(
            m.enumerate_count, count_before,
            "should hit cache within 800ms"
        );
        std::thread::sleep(Duration::from_millis(850));
        let _third = m.enumerate_devices().unwrap();
        assert_eq!(m.enumerate_count, count_before + 1);
    }

    #[test]
    fn mock_empty_keeps_current() {
        let devs = vec![AudioDevice {
            id: "a".into(),
            name: "Sp".into(),
        }];
        let mut m = MockBackend::new(devs.clone(), Some("a".into()));
        let _ = m.enumerate_devices().unwrap();
        m.devices = vec![];
        std::thread::sleep(Duration::from_millis(850));
        let after = m.enumerate_devices().unwrap();
        assert_eq!(after.len(), 1);
    }

    #[test]
    fn clamp_via_backend() {
        let cfg = AppConfig {
            volume_limit: 25,
            volume_limit_enabled: true,
            ..Default::default()
        };
        let mut m = MockBackend::new(vec![], None);
        m.volume = 80;
        m.clamp_volume_if_needed(&cfg).unwrap();
        assert_eq!(m.volume, 25);
    }

    #[test]
    fn set_default_device_mock() {
        let devs = vec![
            AudioDevice {
                id: "a".into(),
                name: "A".into(),
            },
            AudioDevice {
                id: "b".into(),
                name: "B".into(),
            },
        ];
        let mut m = MockBackend::new(devs, Some("a".into()));
        m.set_default_device("b").unwrap();
        assert_eq!(m.default_id.as_deref(), Some("b"));
        assert!(m.set_default_device("c").is_err());
    }

    #[test]
    #[ignore]
    fn integration_real_mock() {
        let devs = vec![AudioDevice {
            id: "x".into(),
            name: "X".into(),
        }];
        let mut backend: Box<dyn AudioBackend> = Box::new(MockBackend::new(devs, None));
        let list = backend.enumerate_devices().unwrap();
        assert_eq!(list.len(), 1);
    }
}

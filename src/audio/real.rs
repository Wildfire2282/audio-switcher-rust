#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    clippy::ptr_as_ptr,
    clippy::borrow_as_ptr
)]
use super::{AudioBackend, AudioDevice, AudioError, AudioSnapshot};
use crate::config::{clamp_volume, AppConfig};
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
#[cfg(windows)]
use windows::core::{Interface, GUID, HRESULT, PCWSTR};
#[cfg(windows)]
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
#[cfg(windows)]
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
#[cfg(windows)]
use windows::Win32::Media::Audio::{
    eMultimedia, eRender, IMMDevice, IMMDeviceCollection, IMMDeviceEnumerator,
    IMMNotificationClient, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
};
#[cfg(windows)]
use windows::Win32::System::Com::StructuredStorage::PropVariantClear;
#[cfg(windows)]
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL, STGM_READ};
#[cfg(windows)]
use windows::Win32::System::Variant::VT_LPWSTR;
#[cfg(windows)]
type SetDefaultEndpointFn =
    unsafe extern "system" fn(*mut std::ffi::c_void, PCWSTR, i32) -> HRESULT;

#[cfg(windows)]
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
    const IID_IPOLICYCONFIG_VISTA: GUID = GUID::from_u128(0x568b9108_44bf_40b4_9006_86afe5b5a620);
    const CANDIDATES: &[(GUID, GUID, usize)] = &[
        (GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9), IID_IPOLICYCONFIG, 13),
        (GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2ea9), IID_IPOLICYCONFIG, 13),
        (GUID::from_u128(0x294935ce_f637_4e7c_a41b_ab255460b862), IID_IPOLICYCONFIG_VISTA, 12),
        (GUID::from_u128(0x294935ce_f588_4bd5_9f8c_bab13166b487), IID_IPOLICYCONFIG_VISTA, 12),
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
        // SAFETY: CoCreateInstance with valid CLSID, no aggregation, CLSCTX_ALL.
        let instance: windows::core::Result<IUnknown> =
            unsafe { CoCreateInstance(&clsid, None, CLSCTX_ALL) };
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
                    let rel: ReleaseFn = unsafe { std::mem::transmute(*vtbl_iface.add(2)) };
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
                    let rel: ReleaseFn = unsafe { std::mem::transmute(*vtbl_iface.add(2)) };
                    unsafe { rel(iface) };
                }
            }
        }
        // 2) QI 失败或环境不支持 QI，回退到直接在 IUnknown 裸指针上按主偏移调用
        //    （多数系统上具体类直接实现接口，裸调同样有效）
        let vtbl = unsafe { *(raw_unk as *mut *mut *mut std::ffi::c_void) };
        if vtbl.is_null() {
            last_err = Some(windows::core::Error::from_hresult(windows::core::HRESULT(
                0x8000_4005_u32 as i32,
            )));
            continue;
        }
        let func: SetDefaultEndpointFn = unsafe { std::mem::transmute(*vtbl.add(primary_off)) };
        let hr = unsafe { func(raw_unk, PCWSTR(wide.as_ptr()), role) };
        if hr.is_ok() {
            return Ok(());
        }
        last_err = Some(windows::core::Error::from(hr));
    }
    Err(last_err.unwrap_or(windows::core::Error::from_hresult(windows::core::HRESULT(
        0x8004_0154_u32 as i32,
    ))))
}

#[cfg(windows)]
static DEVICE_CHANGED: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
static SUPPRESS_NOTIFY: AtomicBool = AtomicBool::new(false);
#[cfg(windows)]
struct SuppressGuard;
#[cfg(windows)]
impl SuppressGuard {
    fn new() -> Self {
        SUPPRESS_NOTIFY.store(true, AtomicOrdering::Release);
        Self
    }
}
#[cfg(windows)]
impl Drop for SuppressGuard {
    fn drop(&mut self) {
        SUPPRESS_NOTIFY.store(false, AtomicOrdering::Release);
    }
}

/// Returns and clears the device-change flag set by `IMMNotificationClient`.
#[cfg(windows)]
pub fn take_device_changed() -> bool {
    DEVICE_CHANGED.swap(false, AtomicOrdering::AcqRel)
}

#[windows_core::implement(windows::Win32::Media::Audio::IMMNotificationClient)]
struct Notifier;

#[cfg(windows)]
impl windows::Win32::Media::Audio::IMMNotificationClient_Impl for Notifier_Impl {
    fn OnDeviceStateChanged(
        &self,
        _device_id: &PCWSTR,
        _new_state: windows::Win32::Media::Audio::DEVICE_STATE,
    ) -> windows::core::Result<()> {
        DEVICE_CHANGED.store(true, AtomicOrdering::Release);
        Ok(())
    }
    fn OnDeviceAdded(&self, _device_id: &PCWSTR) -> windows::core::Result<()> {
        DEVICE_CHANGED.store(true, AtomicOrdering::Release);
        Ok(())
    }
    fn OnDeviceRemoved(&self, _device_id: &PCWSTR) -> windows::core::Result<()> {
        DEVICE_CHANGED.store(true, AtomicOrdering::Release);
        Ok(())
    }
    fn OnDefaultDeviceChanged(
        &self,
        _flow: windows::Win32::Media::Audio::EDataFlow,
        _role: windows::Win32::Media::Audio::ERole,
        _device_id: &PCWSTR,
    ) -> windows::core::Result<()> {
        DEVICE_CHANGED.store(true, AtomicOrdering::Release);
        Ok(())
    }
    fn OnPropertyValueChanged(
        &self,
        _device_id: &PCWSTR,
        _key: &windows::Win32::Foundation::PROPERTYKEY,
    ) -> windows::core::Result<()> {
        if SUPPRESS_NOTIFY.load(AtomicOrdering::Acquire) {
            return Ok(());
        }
        DEVICE_CHANGED.store(true, AtomicOrdering::Release);
        Ok(())
    }
}

#[cfg(windows)]
/// Holder for the COM notification client kept for process lifetime.
/// `Notifier` is stateless and only touches `DEVICE_CHANGED` atomics, so it
/// is effectively `Send`/`Sync` even though COM STA objects are normally
/// thread-affine. We only create/register on the main STA thread, and
/// `OnceLock` only extends lifetime — no cross-thread COM call is made
/// through the holder.
struct NotifierHolder(#[allow(dead_code)] IMMNotificationClient);
#[cfg(windows)]
// SAFETY: Notifier only stores atomics; IMMNotificationClient methods are
// stateless and thread-safe for this implementation. Register is called once
// on the main STA thread; holding the client for lifetime is sound.
unsafe impl Send for NotifierHolder {}
#[cfg(windows)]
// SAFETY: see Send impl.
unsafe impl Sync for NotifierHolder {}
#[cfg(windows)]
static NOTIFIER_HOLDER: std::sync::OnceLock<NotifierHolder> = std::sync::OnceLock::new();

#[cfg(windows)]
fn register_notification_client() {
    if NOTIFIER_HOLDER.get().is_some() {
        return;
    }
    unsafe {
        // SAFETY: CoCreateInstance and RegisterEndpointNotificationCallback are valid on initialized STA thread; NOTIFIER_HOLDER ensures lifetime
        if let Ok(enumerator) =
            CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL)
        {
            let notifier: IMMNotificationClient = Notifier.into();
            let _ = enumerator.RegisterEndpointNotificationCallback(&notifier);
            let _ = NOTIFIER_HOLDER.set(NotifierHolder(notifier));
        }
    }
}

/// Real Windows WASAPI backend.
#[cfg(windows)]
pub struct RealBackend {
    cached: Option<Vec<AudioDevice>>,
    cache_time: Option<Instant>,
    // 复用 enumerator / endpoint 避免重复 CoCreateInstance (启动阶段批量查询)
    cached_enumerator: Option<windows::Win32::Media::Audio::IMMDeviceEnumerator>,
}

#[cfg(windows)]
impl RealBackend {
    /// Create a backend and register the endpoint notification client once.
    pub fn new() -> Self {
        let s = Self { cached: None, cache_time: None, cached_enumerator: None };
        // register once per process
        static REGISTERED: AtomicBool = AtomicBool::new(false);
        if !REGISTERED.swap(true, AtomicOrdering::SeqCst) {
            register_notification_client();
        }
        s
    }

    /// Invalidate the device enumeration cache.
    pub fn clear_cache(&mut self) {
        self.cached = None;
        self.cache_time = None;
        // enumerator 保持可用，无需清除；仅设备列表缓存失效
    }

    /// 获取或创建缓存的 IMMDeviceEnumerator，减少 CoCreateInstance 次数
    fn enumerator_mut(
        &mut self,
    ) -> windows::core::Result<windows::Win32::Media::Audio::IMMDeviceEnumerator> {
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
                            let _guard = SuppressGuard::new();
                            let v = clamped.min(100) as f32 / 100.0;
                            if vol.SetMasterVolumeLevelScalar(v, std::ptr::null()).is_ok() {
                                snap.volume = clamped;
                            }
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
                Self::get_enumerator().map_err(AudioError::from)?
            };
            let dev = enumerator
                .GetDefaultAudioEndpoint(eRender, eMultimedia)
                .map_err(AudioError::from)?;
            let vol: IAudioEndpointVolume =
                dev.Activate(CLSCTX_ALL, None).map_err(AudioError::from)?;
            let scalar = vol.GetMasterVolumeLevelScalar().map_err(AudioError::from)?;
            let m = vol.GetMute().map_err(AudioError::from)?;
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
                .map_err(AudioError::from)?;
            let count = collection.GetCount().map_err(AudioError::from)?;
            let mut devices = Vec::new();
            for i in 0..count {
                if let Ok(dev) = collection.Item(i) {
                    if let Ok(id) = Self::device_id(&dev) {
                        let name = Self::device_friendly_name(&dev);
                        devices.push(AudioDevice { id, name });
                    }
                }
            }
            // Always update cache, even if empty — UI must see removal vs stale list.
            self.cached = Some(devices.clone());
            self.cache_time = Some(Instant::now());
            Ok(devices)
        }
    }

    #[allow(dead_code)]
    /// Polls the notification flag and clears cache if a device changed.
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

#[cfg(windows)]
impl AudioBackend for RealBackend {
    fn fetch_snapshot_clamped(&mut self, cfg: &AppConfig) -> AudioSnapshot {
        RealBackend::fetch_snapshot_clamped(self, cfg)
    }
    fn get_volume_and_mute(&self) -> Result<(u32, bool), AudioError> {
        // optimized: single Activate, reuse cached_enumerator
        unsafe {
            let enumerator = if let Some(e) = &self.cached_enumerator {
                e.clone()
            } else {
                Self::get_enumerator().map_err(AudioError::from)?
            };
            let dev = enumerator
                .GetDefaultAudioEndpoint(eRender, eMultimedia)
                .map_err(AudioError::from)?;
            let vol: IAudioEndpointVolume =
                dev.Activate(CLSCTX_ALL, None).map_err(AudioError::from)?;
            let scalar = vol.GetMasterVolumeLevelScalar().map_err(AudioError::from)?;
            let m = vol.GetMute().map_err(AudioError::from)?;
            Ok(((scalar * 100.0).round() as u32, m.as_bool()))
        }
    }
    fn poll_device_changed(&mut self) -> bool {
        if take_device_changed() {
            self.clear_cache();
            return true;
        }
        false
    }
    fn enumerate_devices(&mut self) -> Result<Vec<AudioDevice>, AudioError> {
        let enumerator = self.enumerator_mut().map_err(AudioError::from)?;
        self.enumerate_devices_inner(&enumerator)
    }

    fn get_default_device(&self) -> Option<AudioDevice> {
        unsafe {
            let enumerator = if let Some(e) = &self.cached_enumerator {
                e.clone()
            } else {
                Self::get_enumerator().ok()?
            };
            let dev = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia).ok()?;
            let id = Self::device_id(&dev).ok()?;
            let name = Self::device_friendly_name(&dev);
            Some(AudioDevice { id, name })
        }
    }

    fn set_default_device(&mut self, id: &str) -> Result<(), AudioError> {
        if id.is_empty() || id.contains('\0') {
            return Err(AudioError::Failed("invalid device id".into()));
        }
        unsafe {
            // Primary role: eMultimedia (0), must succeed.
            set_default_endpoint_raw(id, eMultimedia.0)
                .map_err(|e| AudioError::Failed(e.to_string()))?;
            // Secondary roles: best-effort but log failures (do not hide).
            for role in [0i32, 2i32] {
                if let Err(e) = set_default_endpoint_raw(id, role) {
                    // 0x80070490 = not found, 0x80070057 = invalid arg — don't retry, just warn.
                    eprintln!("set_default role {role} failed: {e}");
                }
            }
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
            let enumerator = self.enumerator_mut().map_err(AudioError::from)?;
            let dev = enumerator
                .GetDefaultAudioEndpoint(eRender, eMultimedia)
                .map_err(AudioError::from)?;
            let vol: IAudioEndpointVolume =
                dev.Activate(CLSCTX_ALL, None).map_err(AudioError::from)?;
            match vol.SetMasterVolumeLevelScalar(v, std::ptr::null()) {
                Ok(()) => Ok(()),
                Err(e) => {
                    // Only retry on busy/timeout HRESULTs; invalid arg is permanent.
                    let hr = e.code().0 as u32;
                    if hr == 0x8007001E || hr == 0x800704D4 {
                        if vol.SetMasterVolumeLevelScalar(v, std::ptr::null()).is_ok() {
                            return Ok(());
                        }
                    }
                    Self::show_msgbox(&format!("设置音量失败: {}", e));
                    Err(AudioError::Failed(e.to_string()))
                }
            }
        }
    }

    fn get_mute(&self) -> Result<bool, AudioError> {
        self.get_volume_and_mute().map(|(_, m)| m)
    }

    fn set_mute(&mut self, mute: bool) -> Result<(), AudioError> {
        unsafe {
            let enumerator = self.enumerator_mut().map_err(AudioError::from)?;
            let dev = enumerator
                .GetDefaultAudioEndpoint(eRender, eMultimedia)
                .map_err(AudioError::from)?;
            let vol: IAudioEndpointVolume =
                dev.Activate(CLSCTX_ALL, None).map_err(AudioError::from)?;
            vol.SetMute(mute, std::ptr::null()).map_err(|e| {
                Self::show_msgbox("切换静音失败");
                AudioError::Failed(e.to_string())
            })
        }
    }

    fn clamp_volume_if_needed(&mut self, cfg: &AppConfig) -> Result<(), AudioError> {
        if !cfg.volume_limit_enabled {
            return Ok(());
        }
        let (vol, _) = self.get_volume_and_mute()?;
        let clamped = clamp_volume(vol, cfg);
        if clamped != vol {
            let _guard = SuppressGuard::new();
            return self.set_volume(clamped);
        }
        Ok(())
    }
}

#[cfg(not(windows))]
use windows::Win32::Foundation::HANDLE;
/// Real backend stub for non-Windows (compilation only).
#[cfg(not(windows))]
pub struct RealBackend {
    cached: Option<Vec<AudioDevice>>,
    cache_time: Option<Instant>,
}
#[cfg(not(windows))]
impl RealBackend {
    /// Create a stub backend.
    pub fn new() -> Self {
        Self { cached: None, cache_time: None }
    }
    /// Invalidate cache (no-op on non-Windows).
    pub fn clear_cache(&mut self) {
        self.cached = None;
        self.cache_time = None;
    }
    /// Poll device change — always false on non-Windows.
    pub fn poll_device_changed(&mut self) -> bool {
        false
    }
    /// Fetch snapshot — default on non-Windows.
    pub fn fetch_snapshot(&mut self) -> AudioSnapshot {
        AudioSnapshot::default()
    }
    /// Fetch clamped snapshot — default on non-Windows.
    pub fn fetch_snapshot_clamped(&mut self, _cfg: &AppConfig) -> AudioSnapshot {
        AudioSnapshot::default()
    }
    /// Get volume and mute — returns `(50, false)` on non-Windows.
    pub fn get_volume_and_mute(&self) -> Result<(u32, bool), AudioError> {
        Ok((50, false))
    }
}
#[cfg(not(windows))]
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
/// Stub — always false on non-Windows.
#[cfg(not(windows))]
pub fn take_device_changed() -> bool {
    false
}
/// Stub — null handle on non-Windows.
#[cfg(not(windows))]
pub fn device_event_handle() -> windows::Win32::Foundation::HANDLE {
    windows::Win32::Foundation::HANDLE::default()
}

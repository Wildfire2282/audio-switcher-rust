//! COM lifecycle RAII.

#[cfg(windows)]
use windows::core::PCWSTR;
#[cfg(windows)]
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};

/// RAII guard for COM — thread-affine (STA) and `!Send`/`!Sync`.
///
/// Contains a `PhantomData<*const ()>` marker so it cannot be sent across
/// threads; `CoUninitialize` must run on the same STA thread that called
/// `CoInitializeEx`.
pub struct ComGuard {
    #[cfg(windows)]
    _private: (),
    /// Marker to make `ComGuard` !Send + !Sync (STA thread-affine).
    _marker: std::marker::PhantomData<*const ()>,
}

impl ComGuard {
    /// Initialize COM (STA).
    ///
    /// On failure shows a message box and returns `None`.
    #[must_use]
    pub fn init() -> Option<Self> {
        #[cfg(windows)]
        {
            // SAFETY: CoInitializeEx is safe to call once per STA thread.
            let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
            if hr.is_err() {
                // SAFETY: MessageBoxW with null-terminated PCWSTRs kept alive for call.
                unsafe {
                    use windows::Win32::UI::WindowsAndMessaging::{
                        MessageBoxW, MB_ICONWARNING, MB_OK,
                    };
                    let msg: Vec<u16> = "COM 初始化失败，程序将退出\0".encode_utf16().collect();
                    let title: Vec<u16> = "Audio Switcher\0".encode_utf16().collect();
                    MessageBoxW(
                        None,
                        PCWSTR(msg.as_ptr()),
                        PCWSTR(title.as_ptr()),
                        MB_OK | MB_ICONWARNING,
                    );
                }
                return None;
            }
        }
        Some(Self {
            #[cfg(windows)]
            _private: (),
            _marker: std::marker::PhantomData,
        })
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            // SAFETY: balances successful CoInitializeEx in init().
            unsafe {
                CoUninitialize();
            }
        }
    }
}

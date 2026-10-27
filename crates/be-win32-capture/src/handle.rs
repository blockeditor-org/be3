use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Threading::{CreateEventW, SetEvent};
use windows::core::PCWSTR;

pub(crate) struct WinEvent(HANDLE);

// unsafe(send-sync)
unsafe impl Send for WinEvent {}
// unsafe(send-sync)
unsafe impl Sync for WinEvent {}

// unsafe(ffi)
impl WinEvent {
    pub(crate) fn new(manual: bool) -> Result<WinEvent, String> {
        unsafe { CreateEventW(None, manual, false, PCWSTR::null()) }
            .map(WinEvent)
            .map_err(|error| format!("an event could not be created: {error}"))
    }

    pub(crate) fn raw(&self) -> HANDLE {
        self.0
    }

    pub(crate) fn set(&self) {
        let _ = unsafe { SetEvent(self.0) };
    }
}

// unsafe(ffi)
impl Drop for WinEvent {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

pub(crate) struct NtHandle(pub(crate) HANDLE);

// unsafe(send-sync)
unsafe impl Send for NtHandle {}
// unsafe(send-sync)
unsafe impl Sync for NtHandle {}

// unsafe(ffi)
impl Drop for NtHandle {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

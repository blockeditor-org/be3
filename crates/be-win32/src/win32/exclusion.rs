use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::System::Threading::{GetCurrentProcessId, GetCurrentThreadId};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumThreadWindows, GA_ROOT, GetAncestor, GetWindowThreadProcessId, SetWindowDisplayAffinity,
    WDA_EXCLUDEFROMCAPTURE, WINEVENT_OUTOFCONTEXT,
};
use windows::core::BOOL;

use crate::events::{CHILDID_SELF, EVENT_OBJECT_CREATE, EVENT_OBJECT_SHOW, OBJID_WINDOW};

static REPORTED: AtomicBool = AtomicBool::new(false);

pub struct CaptureExclusion {
    hook: HWINEVENTHOOK,
}

impl CaptureExclusion {
    // unsafe(ffi)
    pub fn start() -> Self {
        let thread = unsafe { GetCurrentThreadId() };
        let _ = unsafe { EnumThreadWindows(thread, Some(listed), LPARAM(0)) };
        let hook = unsafe {
            SetWinEventHook(
                EVENT_OBJECT_CREATE,
                EVENT_OBJECT_SHOW,
                None,
                Some(on_event),
                GetCurrentProcessId(),
                thread,
                WINEVENT_OUTOFCONTEXT,
            )
        };
        if hook.is_invalid() {
            eprintln!(
                "be-win32: windows Block opens later are not hidden from captures: {}",
                std::io::Error::last_os_error()
            );
        }
        Self { hook }
    }
}

impl Drop for CaptureExclusion {
    // unsafe(ffi)
    fn drop(&mut self) {
        if !self.hook.is_invalid() {
            let _ = unsafe { UnhookWinEvent(self.hook) };
        }
    }
}

// unsafe(ffi)
unsafe extern "system" fn listed(window: HWND, _: LPARAM) -> BOOL {
    exclude(window);
    BOOL(1)
}

// unsafe(ffi)
unsafe extern "system" fn on_event(
    _hook: HWINEVENTHOOK,
    _event: u32,
    window: HWND,
    object: i32,
    child: i32,
    _thread: u32,
    _time: u32,
) {
    if object == OBJID_WINDOW
        && child == CHILDID_SELF
        && !window.0.is_null()
        && unsafe { GetAncestor(window, GA_ROOT) } == window
    {
        exclude(window);
    }
}

// unsafe(ffi)
fn exclude(window: HWND) {
    let owner = unsafe { GetWindowThreadProcessId(window, None) };
    if owner != unsafe { GetCurrentThreadId() } {
        return;
    }
    if let Err(error) = unsafe { SetWindowDisplayAffinity(window, WDA_EXCLUDEFROMCAPTURE) }
        && !REPORTED.swap(true, Ordering::Relaxed)
    {
        eprintln!(
            "be-win32: a window of Block's ({:?}) is not hidden from captures: {error}",
            window.0
        );
    }
}

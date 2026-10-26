use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use windows::Win32::Devices::Display::{
    DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
    DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO,
    DISPLAYCONFIG_SOURCE_DEVICE_NAME, DISPLAYCONFIG_TARGET_DEVICE_NAME, DisplayConfigGetDeviceInfo,
    GetDisplayConfigBufferSizes, QDC_ONLY_ACTIVE_PATHS, QueryDisplayConfig,
};
use windows::Win32::Foundation::{ERROR_SUCCESS, HANDLE, HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    DISPLAY_DEVICEW, EnumDisplayDevicesW, EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR,
    MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::Security::{
    GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TOKEN_MANDATORY_LABEL,
    TOKEN_QUERY, TokenIntegrityLevel,
};
use windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PropVariantToStringAlloc};
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, DESKTOP_CONTROL_FLAGS, DESKTOP_READOBJECTS, GetUserObjectInformationW,
    OpenInputDesktop, UOI_NAME,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentProcessId, OpenProcess, OpenProcessToken, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;
use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, SHGetPropertyStoreForWindow};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GCLP_HICON, GCLP_HICONSM, GW_OWNER, GWL_EXSTYLE, GWL_STYLE, GetClassLongPtrW,
    GetClassNameW, GetForegroundWindow, GetWindow, GetWindowLongPtrW, GetWindowRect,
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, HICON, ICON_BIG, ICON_SMALL,
    ICON_SMALL2, IsHungAppWindow, IsIconic, IsWindowVisible, MONITORINFOF_PRIMARY,
    SMTO_ABORTIFHUNG, SMTO_BLOCK, SendMessageTimeoutW, WM_GETICON, WS_CHILD, WS_EX_APPWINDOW,
    WS_EX_DLGMODALFRAME, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::{BOOL, PCWSTR, PWSTR};

use super::{Owned, from_wide, handle, hwnd, icons};
use crate::{
    Desktop, Handle, Image, InputDesktop, IntegrityLevel, Monitor, Process, RawWindow, Rect, Style,
    monitors,
};

const ICON_TIMEOUT_MS: u32 = 50;

pub struct Win32Desktop {
    own_process: u32,
    own_integrity: Option<IntegrityLevel>,
}

impl Win32Desktop {
    // unsafe(ffi)
    pub fn new() -> Self {
        Self {
            own_process: unsafe { GetCurrentProcessId() },
            own_integrity: integrity(unsafe { GetCurrentProcess() }),
        }
    }
}

impl Default for Win32Desktop {
    fn default() -> Self {
        Self::new()
    }
}

impl Desktop for Win32Desktop {
    // unsafe(ffi)
    fn windows(&self) -> Vec<RawWindow> {
        let mut handles: Vec<HWND> = Vec::new();
        let _ = unsafe {
            EnumWindows(
                Some(collect_window),
                LPARAM(&mut handles as *mut Vec<HWND> as isize),
            )
        };
        handles.into_iter().map(raw_window).collect()
    }

    fn process(&self, process_id: u32) -> Process {
        process(process_id)
    }

    fn icon(&self, window: Handle) -> Option<Arc<Image>> {
        icon(hwnd(window)).map(Arc::new)
    }

    fn app_user_model_id(&self, window: Handle) -> Option<String> {
        app_user_model_id(hwnd(window))
    }

    fn monitors(&self) -> Vec<Monitor> {
        monitors()
    }

    // unsafe(ffi)
    fn foreground(&self) -> Option<Handle> {
        let window = unsafe { GetForegroundWindow() };
        (!window.0.is_null()).then(|| handle(window))
    }

    fn input_desktop(&self) -> InputDesktop {
        InputDesktop::from_name(input_desktop_name().as_deref())
    }

    fn own_process_id(&self) -> u32 {
        self.own_process
    }

    fn own_integrity(&self) -> Option<IntegrityLevel> {
        self.own_integrity
    }
}

// unsafe(ffi)
unsafe extern "system" fn collect_window(window: HWND, list: LPARAM) -> BOOL {
    let list = unsafe { &mut *(list.0 as *mut Vec<HWND>) };
    list.push(window);
    BOOL(1)
}

// unsafe(ffi)
fn raw_window(window: HWND) -> RawWindow {
    let mut process_id = 0;
    let thread_id = unsafe { GetWindowThreadProcessId(window, Some(&mut process_id)) };
    let owner = unsafe { GetWindow(window, GW_OWNER) }.ok().map(handle);
    let mut class = [0u16; 256];
    let length = unsafe { GetClassNameW(window, &mut class) };
    let class = String::from_utf16_lossy(&class[..usize::try_from(length).unwrap_or(0)]);
    let length = unsafe { GetWindowTextLengthW(window) };
    let mut title = vec![0u16; usize::try_from(length).unwrap_or(0) + 1];
    let length = unsafe { GetWindowTextW(window, &mut title) };
    let title = String::from_utf16_lossy(&title[..usize::try_from(length).unwrap_or(0)]);
    let style = unsafe { GetWindowLongPtrW(window, GWL_STYLE) } as u32;
    let extended = unsafe { GetWindowLongPtrW(window, GWL_EXSTYLE) } as u32;
    let mut cloaked = 0u32;
    let cloaked = unsafe {
        DwmGetWindowAttribute(
            window,
            DWMWA_CLOAKED,
            (&mut cloaked as *mut u32).cast(),
            size_of::<u32>() as u32,
        )
    }
    .is_ok()
        && cloaked != 0;
    let mut frame = RECT::default();
    let read = unsafe {
        DwmGetWindowAttribute(
            window,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&mut frame as *mut RECT).cast(),
            size_of::<RECT>() as u32,
        )
    };
    if read.is_err() {
        let _ = unsafe { GetWindowRect(window, &mut frame) };
    }
    RawWindow {
        handle: handle(window),
        owner,
        process_id,
        thread_id,
        class,
        title,
        visible: unsafe { IsWindowVisible(window) }.as_bool(),
        cloaked,
        enabled: unsafe { IsWindowEnabled(window) }.as_bool(),
        minimized: unsafe { IsIconic(window) }.as_bool(),
        style: Style {
            popup: style & WS_POPUP.0 != 0,
            child: style & WS_CHILD.0 != 0,
            tool_window: extended & WS_EX_TOOLWINDOW.0 != 0,
            app_window: extended & WS_EX_APPWINDOW.0 != 0,
            no_activate: extended & WS_EX_NOACTIVATE.0 != 0,
            dialog_frame: extended & WS_EX_DLGMODALFRAME.0 != 0,
        },
        frame: rect(frame),
        responding: !unsafe { IsHungAppWindow(window) }.as_bool(),
    }
}

fn rect(rect: RECT) -> Rect {
    Rect::new(rect.left, rect.top, rect.right, rect.bottom)
}

// unsafe(ffi)
fn process(process_id: u32) -> Process {
    let Ok(process) =
        (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) })
    else {
        return Process::default();
    };
    let process = Owned(process);
    let mut path = vec![0u16; 32768];
    let mut length = path.len() as u32;
    let exe = unsafe {
        QueryFullProcessImageNameW(
            process.0,
            PROCESS_NAME_WIN32,
            PWSTR(path.as_mut_ptr()),
            &mut length,
        )
    }
    .ok()
    .map(|()| PathBuf::from(String::from_utf16_lossy(&path[..length as usize])));
    Process {
        exe,
        integrity: integrity(process.0),
    }
}

// unsafe(ffi)
fn integrity(process: HANDLE) -> Option<IntegrityLevel> {
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) }.ok()?;
    let token = Owned(token);
    let mut length = 0u32;
    let _ = unsafe { GetTokenInformation(token.0, TokenIntegrityLevel, None, 0, &mut length) };
    if length == 0 {
        return None;
    }
    let mut buffer = vec![0u64; (length as usize).div_ceil(8)];
    unsafe {
        GetTokenInformation(
            token.0,
            TokenIntegrityLevel,
            Some(buffer.as_mut_ptr().cast()),
            length,
            &mut length,
        )
    }
    .ok()?;
    let label = unsafe { &*buffer.as_ptr().cast::<TOKEN_MANDATORY_LABEL>() };
    let sid = label.Label.Sid;
    let count = unsafe { *GetSidSubAuthorityCount(sid) };
    let last = u32::from(count.checked_sub(1)?);
    Some(IntegrityLevel(unsafe { *GetSidSubAuthority(sid, last) }))
}

// unsafe(ffi)
fn icon(window: HWND) -> Option<Image> {
    for kind in [ICON_BIG, ICON_SMALL2, ICON_SMALL] {
        let mut result = 0usize;
        let sent = unsafe {
            SendMessageTimeoutW(
                window,
                WM_GETICON,
                WPARAM(kind as usize),
                LPARAM(0),
                SMTO_ABORTIFHUNG | SMTO_BLOCK,
                ICON_TIMEOUT_MS,
                Some(&mut result),
            )
        };
        if sent.0 != 0 && result != 0 {
            return icons::from_hicon(HICON(result as *mut core::ffi::c_void));
        }
    }
    [GCLP_HICON, GCLP_HICONSM].into_iter().find_map(|index| {
        let icon = unsafe { GetClassLongPtrW(window, index) };
        (icon != 0)
            .then(|| icons::from_hicon(HICON(icon as *mut core::ffi::c_void)))
            .flatten()
    })
}

// unsafe(ffi)
fn app_user_model_id(window: HWND) -> Option<String> {
    let store: IPropertyStore = unsafe { SHGetPropertyStoreForWindow(window) }.ok()?;
    property_string(&store)
}

// unsafe(ffi)
pub(super) fn property_string(store: &IPropertyStore) -> Option<String> {
    let mut value = unsafe { store.GetValue(&PKEY_AppUserModel_ID) }.ok()?;
    let text = unsafe { PropVariantToStringAlloc(&value) }.ok();
    let _ = unsafe { PropVariantClear(&mut value) };
    let text = text?;
    let string = unsafe { text.to_string() }.ok();
    unsafe { CoTaskMemFree(Some(text.0 as *const core::ffi::c_void)) };
    string.filter(|id| !id.is_empty())
}

// unsafe(ffi)
fn input_desktop_name() -> Option<String> {
    let desktop =
        unsafe { OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_READOBJECTS) }.ok()?;
    let mut name = [0u16; 256];
    let read = unsafe {
        GetUserObjectInformationW(
            HANDLE(desktop.0),
            UOI_NAME,
            Some(name.as_mut_ptr().cast()),
            size_of_val(&name) as u32,
            None,
        )
    };
    let _ = unsafe { CloseDesktop(desktop) };
    read.ok().map(|()| from_wide(&name))
}

// unsafe(ffi)
unsafe extern "system" fn collect_monitor(
    monitor: HMONITOR,
    _: HDC,
    _: *mut RECT,
    list: LPARAM,
) -> BOOL {
    let list = unsafe { &mut *(list.0 as *mut Vec<HMONITOR>) };
    list.push(monitor);
    BOOL(1)
}

// unsafe(ffi)
fn monitors() -> Vec<Monitor> {
    let mut handles: Vec<HMONITOR> = Vec::new();
    let _ = unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(collect_monitor),
            LPARAM(&mut handles as *mut Vec<HMONITOR> as isize),
        )
    };
    let friendly = friendly_names();
    let mut list: Vec<Monitor> = handles
        .into_iter()
        .enumerate()
        .filter_map(|(index, monitor)| {
            let mut info = MONITORINFOEXW::default();
            info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
            if !unsafe {
                GetMonitorInfoW(
                    monitor,
                    (&mut info as *mut MONITORINFOEXW).cast::<MONITORINFO>(),
                )
            }
            .as_bool()
            {
                return None;
            }
            let device = from_wide(&info.szDevice);
            let mut adapter = DISPLAY_DEVICEW {
                cb: size_of::<DISPLAY_DEVICEW>() as u32,
                ..Default::default()
            };
            let description =
                unsafe { EnumDisplayDevicesW(PCWSTR(info.szDevice.as_ptr()), 0, &mut adapter, 0) }
                    .as_bool()
                    .then(|| from_wide(&adapter.DeviceString));
            let (mut dpi_x, mut dpi_y) = (96u32, 96u32);
            let _ = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
            Some(Monitor {
                handle: Handle(monitor.0 as usize as u64),
                name: monitors::name(
                    friendly.get(&device).map(String::as_str),
                    description.as_deref(),
                    index,
                ),
                device,
                rect: rect(info.monitorInfo.rcMonitor),
                work_area: rect(info.monitorInfo.rcWork),
                scale: dpi_x as f32 / 96.0,
                primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
            })
        })
        .collect();
    monitors::sort(&mut list);
    list
}

// unsafe(ffi)
fn friendly_names() -> HashMap<String, String> {
    let mut names = HashMap::new();
    let (mut path_count, mut mode_count) = (0u32, 0u32);
    if unsafe {
        GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count)
    } != ERROR_SUCCESS
    {
        return names;
    }
    let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
    let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
    let queried = unsafe {
        QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut path_count,
            paths.as_mut_ptr(),
            &mut mode_count,
            modes.as_mut_ptr(),
            None,
        )
    };
    if queried != ERROR_SUCCESS {
        return names;
    }
    for path in &paths[..path_count as usize] {
        let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
            header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                size: size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                adapterId: path.sourceInfo.adapterId,
                id: path.sourceInfo.id,
            },
            ..Default::default()
        };
        let mut target = DISPLAYCONFIG_TARGET_DEVICE_NAME {
            header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
                size: size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32,
                adapterId: path.targetInfo.adapterId,
                id: path.targetInfo.id,
            },
            ..Default::default()
        };
        if unsafe { DisplayConfigGetDeviceInfo(&mut source.header) } == 0
            && unsafe { DisplayConfigGetDeviceInfo(&mut target.header) } == 0
        {
            names.insert(
                from_wide(&source.viewGdiDeviceName),
                from_wide(&target.monitorFriendlyDeviceName),
            );
        }
    }
    names
}

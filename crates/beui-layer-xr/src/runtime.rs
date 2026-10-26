use std::path::{Path, PathBuf};

use openxr as xr;

pub struct Loaded {
    pub entry: xr::Entry,
    pub _library: Option<libloading::Library>,
}

// unsafe(ffi)
pub fn load() -> Result<Loaded, String> {
    if let Ok(entry) = unsafe { xr::Entry::load(&()) } {
        return Ok(Loaded {
            entry,
            _library: None,
        });
    }
    let manifest = active_runtime()
        .ok_or("no OpenXR loader was found, and no active OpenXR runtime is set")?;
    let library = runtime_library(&manifest)?;
    let library = unsafe { libloading::Library::new(&library) }.map_err(|error| {
        format!(
            "the OpenXR runtime {} did not load: {error}",
            library.display()
        )
    })?;
    let negotiate: libloading::Symbol<'_, xr::sys::pfn::NegotiateLoaderRuntimeInterface> = unsafe {
        library.get(b"xrNegotiateLoaderRuntimeInterface\0")
    }
    .map_err(|error| format!("the OpenXR runtime has no negotiation entry point: {error}"))?;
    let info = xr::sys::NegotiateLoaderInfo {
        struct_type: xr::sys::NegotiateLoaderInfo::TYPE,
        struct_version: xr::sys::NegotiateLoaderInfo::VERSION,
        struct_size: size_of::<xr::sys::NegotiateLoaderInfo>(),
        min_interface_version: 1,
        max_interface_version: 1,
        min_api_version: xr::Version::new(1, 0, 0),
        max_api_version: xr::Version::new(1, 0x3ff, 0xfff),
    };
    let mut request = xr::sys::NegotiateRuntimeRequest {
        struct_type: xr::sys::NegotiateRuntimeRequest::TYPE,
        struct_version: xr::sys::NegotiateRuntimeRequest::VERSION,
        struct_size: size_of::<xr::sys::NegotiateRuntimeRequest>(),
        runtime_interface_version: 0,
        runtime_api_version: xr::Version::new(0, 0, 0),
        get_instance_proc_addr: None,
    };
    let result = unsafe { negotiate(&info, &mut request) };
    let get_instance_proc_addr = match (result, request.get_instance_proc_addr) {
        (xr::sys::Result::SUCCESS, Some(get)) => get,
        (result, _) => {
            return Err(format!(
                "the OpenXR runtime refused to negotiate: {result:?}"
            ));
        }
    };
    drop(negotiate);
    let entry = unsafe { xr::Entry::from_get_instance_proc_addr(get_instance_proc_addr, &()) }
        .map_err(|error| format!("the OpenXR runtime's entry points did not load: {error}"))?;
    Ok(Loaded {
        entry,
        _library: Some(library),
    })
}

fn runtime_library(manifest: &Path) -> Result<PathBuf, String> {
    let text = std::fs::read_to_string(manifest)
        .map_err(|error| format!("{}: {error}", manifest.display()))?;
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", manifest.display()))?;
    let library = json
        .pointer("/runtime/library_path")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("{} names no runtime.library_path", manifest.display()))?;
    let library = Path::new(library);
    if library.is_absolute() || library.components().count() == 1 {
        return Ok(library.to_owned());
    }
    Ok(manifest
        .parent()
        .map_or_else(|| library.to_owned(), |directory| directory.join(library)))
}

fn active_runtime() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("XR_RUNTIME_JSON") {
        return Some(PathBuf::from(path));
    }
    platform_runtime()
}

#[cfg(target_os = "linux")]
fn platform_runtime() -> Option<PathBuf> {
    let home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")));
    let dirs = std::env::var("XDG_CONFIG_DIRS").unwrap_or_else(|_| "/etc/xdg".to_owned());
    home.into_iter()
        .chain(
            dirs.split(':')
                .filter(|dir| !dir.is_empty())
                .map(PathBuf::from),
        )
        .chain([PathBuf::from("/etc")])
        .map(|dir| dir.join("openxr/1/active_runtime.json"))
        .find(|path| path.is_file())
}

#[cfg(windows)]
fn platform_runtime() -> Option<PathBuf> {
    windows::active_runtime()
}

#[cfg(windows)]
mod windows {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::PathBuf;

    const HKEY_LOCAL_MACHINE: isize = 0x8000_0002_u32 as i32 as isize;
    const RRF_RT_REG_SZ: u32 = 0x0000_0002;

    // unsafe(ffi)
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegGetValueW(
            key: isize,
            subkey: *const u16,
            value: *const u16,
            flags: u32,
            kind: *mut u32,
            data: *mut core::ffi::c_void,
            size: *mut u32,
        ) -> i32;
    }

    fn wide(text: &str) -> Vec<u16> {
        std::ffi::OsStr::new(text)
            .encode_wide()
            .chain([0])
            .collect()
    }

    // unsafe(ffi)
    pub fn active_runtime() -> Option<PathBuf> {
        let subkey = wide("SOFTWARE\\Khronos\\OpenXR\\1");
        let value = wide("ActiveRuntime");
        let mut size = 0u32;
        let status = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                subkey.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut size,
            )
        };
        if status != 0 || size == 0 {
            return None;
        }
        let mut data = vec![0u16; size as usize / 2 + 1];
        let status = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                subkey.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                data.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if status != 0 {
            return None;
        }
        let end = data
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(data.len());
        Some(PathBuf::from(OsString::from_wide(&data[..end])))
    }
}

use std::{
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
};

use super::Plugins;

pub(super) fn scan() -> Plugins {
    let mut scanning = pin!(super::discover(|path| std::future::ready(read(&path))));
    match scanning
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(plugins) => plugins,
        Poll::Pending => unreachable!("reading the bundle never waits"),
    }
}

pub(super) fn fetch(path: String, deliver: impl FnOnce(Result<Vec<u8>, String>) + Send + 'static) {
    deliver(read(&path));
}

#[cfg(not(target_os = "android"))]
pub(crate) fn read(path: &str) -> Result<Vec<u8>, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let root = executable
        .parent()
        .ok_or("the executable is in no directory")?;
    std::fs::read(root.join(path)).map_err(|error| error.to_string())
}

#[cfg(target_os = "android")]
static ASSETS: std::sync::OnceLock<beui_host::native::AndroidApp> = std::sync::OnceLock::new();

#[cfg(target_os = "android")]
pub(crate) fn use_assets(app: beui_host::native::AndroidApp) {
    let _ = ASSETS.set(app);
    if let Ok(id) = read(build_id::FILE) {
        let _ = build_id::install_metadata(&String::from_utf8_lossy(&id));
    }
}

#[cfg(target_os = "android")]
pub(crate) fn read(path: &str) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let app = ASSETS.get().ok_or("the app's assets were not given")?;
    let name =
        std::ffi::CString::new(path).map_err(|_| "the asset path is not a C string".to_owned())?;
    let mut asset = app.asset_manager().open(&name).ok_or("no such asset")?;
    let mut bytes = Vec::new();
    asset
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    Ok(bytes)
}

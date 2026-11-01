use std::{
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
};

use block_wasm_host::Stored;

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

fn read(path: &str) -> Result<Vec<u8>, String> {
    open(path)?.into_bytes()
}

#[cfg(not(target_os = "android"))]
pub fn open(path: &str) -> Result<Stored, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let root = executable
        .parent()
        .ok_or("the executable is in no directory")?;
    Stored::file(root.join(path))
}

#[cfg(target_os = "android")]
type Assets = Box<dyn Fn(&str) -> Result<Vec<u8>, String> + Send + Sync>;

#[cfg(target_os = "android")]
static ASSETS: std::sync::OnceLock<Assets> = std::sync::OnceLock::new();

#[cfg(target_os = "android")]
pub fn use_assets(read_asset: impl Fn(&str) -> Result<Vec<u8>, String> + Send + Sync + 'static) {
    let _ = ASSETS.set(Box::new(read_asset));
    if let Ok(id) = read(build_id::FILE) {
        let _ = build_id::install_metadata(&String::from_utf8_lossy(&id));
    }
}

#[cfg(target_os = "android")]
pub fn open(path: &str) -> Result<Stored, String> {
    let read_asset = ASSETS.get().ok_or("the app's assets were not given")?;
    read_asset(path).map(Stored::Bytes)
}

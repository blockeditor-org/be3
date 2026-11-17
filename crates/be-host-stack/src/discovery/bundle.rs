use std::{
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
};

use be_wasm_host::Stored;

use super::Apps;

pub(super) fn scan() -> Apps {
    let mut scanning = pin!(super::fetch_all(|path| std::future::ready(read(&path))));
    match scanning
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(read) => super::settle(&read),
        Poll::Pending => unreachable!("reading the bundle never waits"),
    }
}

pub(super) fn fetch(path: String, deliver: impl FnOnce(Result<Vec<u8>, String>) + Send + 'static) {
    deliver(read(&path));
}

fn read(path: &str) -> Result<Vec<u8>, String> {
    open(path)?.into_bytes()
}

type Assets = Box<dyn Fn(&str) -> Result<Vec<u8>, String> + Send + Sync>;

static ASSETS: std::sync::OnceLock<Assets> = std::sync::OnceLock::new();

/// Reads the staged apps through `read_asset` rather than from the bundle beside the
/// executable: Android's assets, or a deployed `be`'s release, downloaded as it is read
/// (be-tool-cli's `release`). Called before anything discovers the apps.
pub fn use_assets(read_asset: impl Fn(&str) -> Result<Vec<u8>, String> + Send + Sync + 'static) {
    let first = ASSETS.set(Box::new(read_asset)).is_ok();
    debug_assert!(
        !first
            || super::APPS
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_none(),
        "use_assets after the apps were discovered from the bundle, which they stay"
    );
    if let Ok(id) = read(be_lib_build_id::FILE) {
        let _ = be_lib_build_id::install_metadata(&String::from_utf8_lossy(&id));
    }
}

pub fn open(path: &str) -> Result<Stored, String> {
    match ASSETS.get() {
        Some(read_asset) => read_asset(path).map(Stored::Bytes),
        #[cfg(target_os = "android")]
        None => Err("the host's assets were not given".to_owned()),
        #[cfg(not(target_os = "android"))]
        None => Stored::file(be_lib_build_id::bundle_file(path)?),
    }
}

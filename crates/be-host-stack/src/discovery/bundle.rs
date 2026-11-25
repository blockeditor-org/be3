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

/// Assets a test reads the staged files from while it holds them (`assets_for_testing`), ahead
/// of `use_assets`'s, which a process sets once.
#[cfg(not(target_arch = "wasm32"))]
static TESTING_ASSETS: std::sync::RwLock<Option<std::sync::Arc<Assets>>> =
    std::sync::RwLock::new(None);

/// Reads the staged files through `read_asset` while the answer is held, as `use_assets` does
/// for a process: for a test that shares its process with others which discovered the apps
/// already (`install_for_testing` puts its manifests in place the same way).
#[cfg(not(target_arch = "wasm32"))]
#[must_use = "the assets are read only while this is held"]
pub fn assets_for_testing(
    read_asset: impl Fn(&str) -> Result<Vec<u8>, String> + Send + Sync + 'static,
) -> AssetsForTesting {
    let assets: Assets = Box::new(read_asset);
    let before = TESTING_ASSETS
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .replace(std::sync::Arc::new(assets));
    AssetsForTesting(before)
}

/// Assets a test put in place (`assets_for_testing`), until it is dropped.
#[cfg(not(target_arch = "wasm32"))]
pub struct AssetsForTesting(Option<std::sync::Arc<Assets>>);

#[cfg(not(target_arch = "wasm32"))]
impl Drop for AssetsForTesting {
    fn drop(&mut self) {
        *TESTING_ASSETS
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = self.0.take();
    }
}

pub fn open(path: &str) -> Result<Stored, String> {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(read_asset) = TESTING_ASSETS
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
    {
        return read_asset(path).map(Stored::Bytes);
    }
    match ASSETS.get() {
        Some(read_asset) => read_asset(path).map(Stored::Bytes),
        #[cfg(target_os = "android")]
        None => Err("the host's assets were not given".to_owned()),
        #[cfg(not(target_os = "android"))]
        None => Stored::file(be_lib_build_id::bundle_file(path)?),
    }
}

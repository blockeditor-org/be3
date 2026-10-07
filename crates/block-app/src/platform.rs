use std::sync::mpsc::Receiver;

mod file_saver;
pub(crate) mod http;
#[cfg(not(target_arch = "wasm32"))]
mod native;
mod utc_offset;
#[cfg(target_arch = "wasm32")]
mod web;

pub(crate) use file_saver::{SavedFile, save_file};
#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) use native::start_embedded_server_at;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::{EmbeddedServer, spawn_request, start_embedded_server};
pub(crate) use utc_offset::utc_offset;
#[cfg(target_arch = "wasm32")]
pub(crate) use web::spawn_request;

pub(crate) const HAS_EMBEDDED_SERVER: bool = cfg!(not(target_arch = "wasm32"));

pub(crate) type RequestResult<T> = Receiver<T>;

pub(crate) struct Deliver<T> {
    deliver: Option<Box<dyn FnOnce(T) + Send>>,
    otherwise: Option<T>,
}

impl<T> Deliver<T> {
    pub(crate) fn new(otherwise: T, deliver: impl FnOnce(T) + Send + 'static) -> Self {
        Self {
            deliver: Some(Box::new(deliver)),
            otherwise: Some(otherwise),
        }
    }

    pub(crate) fn send(mut self, value: T) {
        if let Some(deliver) = self.deliver.take() {
            deliver(value);
        }
    }
}

impl<T> Drop for Deliver<T> {
    fn drop(&mut self) {
        if let (Some(deliver), Some(otherwise)) = (self.deliver.take(), self.otherwise.take()) {
            deliver(otherwise);
        }
    }
}

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
pub(crate) fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    let spawned = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let spawned = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let spawned = std::process::Command::new("xdg-open").arg(url).spawn();
    if let Err(error) = spawned {
        eprintln!("could not open {url}: {error}");
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn open_url(url: &str) {
    if let Some(window) = web_sys::window() {
        let _ = window.open_with_url_and_target(url, "_blank");
    }
}

#[cfg(target_os = "android")]
pub(crate) fn open_url(url: &str) {
    eprintln!("opening links is not supported on Android: {url}");
}

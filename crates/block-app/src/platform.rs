use std::sync::mpsc::Receiver;

mod file_picker;
pub(crate) mod http;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_os = "android")]
mod safe_area;
#[cfg(target_arch = "wasm32")]
mod web;

pub(crate) use file_picker::{FileFilter, FilePicker};
#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) use native::start_embedded_server_at;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::{EmbeddedServer, spawn_request, start_embedded_server};
#[cfg(target_arch = "wasm32")]
pub(crate) use web::spawn_request;

pub(crate) const HAS_EMBEDDED_SERVER: bool = cfg!(not(target_arch = "wasm32"));

pub(crate) type RequestResult<T> = Receiver<T>;

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

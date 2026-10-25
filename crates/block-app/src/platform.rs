use std::sync::mpsc::Receiver;

pub(crate) mod http;
#[cfg(not(target_arch = "wasm32"))]
mod native;
mod utc_offset;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) use native::start_embedded_server_at;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::{EmbeddedServer, spawn_request, start_embedded_server};
pub(crate) use utc_offset::utc_offset;
#[cfg(target_arch = "wasm32")]
pub(crate) use web::spawn_request;

pub(crate) const HAS_EMBEDDED_SERVER: bool = cfg!(not(target_arch = "wasm32"));

pub(crate) const SENDS_HEADERS: bool = cfg!(not(target_arch = "wasm32"));

pub(crate) type RequestResult<T> = Receiver<T>;

pub(crate) fn default_account_server() -> Option<build_id::DefaultServer> {
    build_id::metadata().ok()?.default_server.clone()
}

mod browser;
pub(crate) use browser::open_browser;

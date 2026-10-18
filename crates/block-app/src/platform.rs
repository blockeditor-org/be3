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

pub(crate) const SENDS_HEADERS: bool = cfg!(not(target_arch = "wasm32"));

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

pub(crate) fn default_account_server() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_default();
        if origin == "https://be3.pfg.pw" {
            "https://api.be3.pfg.pw".to_owned()
        } else {
            origin
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    String::new()
}

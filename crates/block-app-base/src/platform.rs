pub mod http;
#[cfg(not(target_arch = "wasm32"))]
mod native;
mod utc_offset;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
pub use native::{Request, spawn_request};
pub use utc_offset::utc_offset;
#[cfg(target_arch = "wasm32")]
pub use web::{Request, spawn_request};

pub const HAS_EMBEDDED_SERVER: bool = cfg!(not(target_arch = "wasm32"));

pub const SENDS_HEADERS: bool = cfg!(not(target_arch = "wasm32"));

pub fn default_account_server() -> Option<build_id::DefaultServer> {
    build_id::metadata().ok()?.default_server.clone()
}

mod browser;
pub use browser::open_browser;

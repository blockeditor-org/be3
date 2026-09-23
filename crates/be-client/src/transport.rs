#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::{Reader, Writer, connect, spawn};
#[cfg(target_arch = "wasm32")]
pub(crate) use web::{Reader, Writer, connect, spawn};

pub(crate) enum Frame {
    Binary(Vec<u8>),
    #[cfg(not(target_arch = "wasm32"))]
    Ping(Vec<u8>),
    Closed,
}

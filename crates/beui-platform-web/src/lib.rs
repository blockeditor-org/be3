#[cfg(any(target_arch = "wasm32", test))]
mod lock;

#[cfg(target_arch = "wasm32")]
mod file_picker;
#[cfg(target_arch = "wasm32")]
mod file_saver;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
pub use web::{Web, accessibility_tree, request_frame};

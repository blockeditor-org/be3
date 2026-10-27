pub mod ime_mirror;
pub mod keys;
pub mod wheel;

#[cfg(target_arch = "wasm32")]
mod agent;

#[cfg(target_arch = "wasm32")]
pub use agent::text_agent;

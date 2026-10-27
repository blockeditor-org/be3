#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
pub use web::*;

#[cfg(not(target_arch = "wasm32"))]
mod host {
    use std::time::Duration;

    use crate::node::NodeId;

    pub fn connected(_node: NodeId) -> bool {
        false
    }

    pub fn request_animation_frame(_frame: fn()) {}

    pub fn set_timeout(_delay: Duration, _wake: fn()) {}
}

#[cfg(not(target_arch = "wasm32"))]
pub use host::*;

#[cfg(not(target_arch = "wasm32"))]
mod changes;
mod compare;
mod fingerprint;
mod format;
mod highlight;
mod raster;

#[cfg(not(target_arch = "wasm32"))]
pub use changes::{Change, changes, root};
pub use compare::{Difference, difference, differences};
pub use fingerprint::fingerprint;
pub use format::{
    Content, Frame, Glyph, Primitive, RoundedRect, Snapshot, Texture, TextureKey, Triangle, Turn,
    Vertex,
};
pub use highlight::{Highlight, highlight};
pub use raster::render;

#[cfg(test)]
mod tests;

mod compare;
mod fingerprint;
mod format;
mod highlight;
mod raster;

pub use compare::{Difference, difference};
pub use fingerprint::fingerprint;
pub use format::{
    Content, Frame, Glyph, Primitive, RoundedRect, Snapshot, Texture, TextureKey, Triangle, Turn,
    Vertex,
};
pub use highlight::{Highlight, highlight};
pub use raster::render;

#[cfg(test)]
mod tests;

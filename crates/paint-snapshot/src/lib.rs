mod capture;
mod compare;
mod format;
mod raster;

pub use capture::{TextureStore, capture, fingerprint};
pub use compare::{Difference, difference};
pub use format::{
    Content, Frame, Glyph, Primitive, RoundedRect, Snapshot, Texture, TextureKey, Triangle, Turn,
    Vertex,
};
pub use raster::render;

#[cfg(test)]
mod tests;

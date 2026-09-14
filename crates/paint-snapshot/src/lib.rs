mod capture;
mod compare;
mod format;
mod raster;

pub use capture::{TextureStore, capture};
pub use compare::{Difference, difference};
pub use format::{Content, Frame, Primitive, Snapshot, Texture, TextureKey, Triangle, Vertex};
pub use raster::render;

#[cfg(test)]
mod tests;

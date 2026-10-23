mod browser;
pub use browser::{BrowserFile, BrowserSnapshot};
mod accepted;
mod compare;
mod fingerprint;
mod format;
mod highlight;
mod raster;

pub use accepted::assert_accepted;
pub use compare::{Difference, difference};
pub use fingerprint::fingerprint;
pub use format::{Snapshot, Texture};
pub use highlight::{Highlight, highlight};
pub use paint_format::{
    Content, Fade, Frame, Glyph, Painting, Pixels, Primitive, RoundedRect, TextureKey, Triangle,
    Turn, Vertex,
};
pub use raster::render;

#[cfg(test)]
mod tests;

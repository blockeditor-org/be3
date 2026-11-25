mod browser;
pub use browser::{BrowserFile, BrowserSnapshot};
mod accepted;
mod compare;
mod fingerprint;
mod format;
mod highlight;
mod painting;

pub use accepted::{HOW_TO_ACCEPT, assert_accepted, assert_painting, changed};
pub use beui_paint_format::{
    Content, Fade, Frame, Glyph, Pixels, Primitive, RoundedRect, TextureKey, Triangle, Turn, Vertex,
};
pub use compare::{Difference, difference, frame_difference};
pub use fingerprint::fingerprint;
pub use format::{Snapshot, Texture};
pub use highlight::{Highlight, highlight};
pub use painting::{Native, Painting};

#[cfg(test)]
mod tests;

//! What the PDF app and its workers say to each other (guides/adding_an_app.md, Workers). Its one
//! worker, the renderer (crates/be-worker-pdf-render), renders a page, or the part of one shown,
//! with PDFium, off the editor's own thread: the app sends it a `Job` and gets back an `Answer`
//! for each, in order, the page's pixels as RGBA, which the app uploads. The app depends on this
//! crate alone, so it links no PDFium.

use std::time::Duration;

use be_lib_geometry::{Pos2, Vec2};
use serde::{Deserialize, Serialize};

/// The renderer's module, as the PDF app's manifest names it under `workers`.
pub const MODULE: &str = "render.wasm";

/// The widest or tallest a rendered tile is, in pixels.
pub const DETAIL_MAX_DIM: f32 = 4096.0;
/// The fewest pixels a page is rendered at per PDF point.
pub const MIN_SCALE: f32 = 0.01;

/// What to render of a page.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Target {
    /// The whole page, as large as fits in `max_width` by `max_height` pixels.
    FullPage { max_width: f32, max_height: f32 },
    /// The part of the page at `origin_pts` of `size_pts`, at `scale` pixels per PDF point.
    ShownView {
        scale: f32,
        origin_pts: Pos2,
        size_pts: Vec2,
    },
}

/// What the app asks: render `target` of page `page` of the PDF `data`. `requester` is the
/// app's own, and comes back with the answer: every PDF the app shows shares one renderer.
#[derive(Serialize, Deserialize)]
pub struct Job {
    pub requester: u64,
    pub page: usize,
    pub target: Target,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

/// A rendered tile of a page: `width` by `height` RGBA pixels covering `size_pts` from
/// `origin_pts`, and what the renderer learned of the document.
#[derive(Serialize, Deserialize)]
pub struct Rendered {
    pub page_count: usize,
    pub page_index: usize,
    pub page_size_pts: Vec2,
    pub scale: f32,
    pub origin_pts: Pos2,
    pub size_pts: Vec2,
    pub width: u32,
    pub height: u32,
    #[serde(with = "serde_bytes")]
    pub rgba: Vec<u8>,
}

/// What the renderer measured while it rendered, for the app's performance report.
#[derive(Serialize, Deserialize)]
pub enum Measure {
    Duration(String, Duration),
    Count(String, u64),
}

/// The answer to one `Job`: the tile, or why it could not be rendered.
#[derive(Serialize, Deserialize)]
pub struct Answer {
    pub requester: u64,
    pub result: Result<Rendered, String>,
    pub measures: Vec<Measure>,
}

impl Job {
    pub fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).expect("a job always encodes")
    }

    pub fn decode(message: &[u8]) -> Result<Self, String> {
        postcard::from_bytes(message)
            .map_err(|error| format!("the PDF editor sent a job that did not decode: {error}"))
    }
}

impl Answer {
    pub fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).expect("an answer always encodes")
    }

    pub fn decode(message: &[u8]) -> Result<Self, String> {
        postcard::from_bytes(message)
            .map_err(|error| format!("the PDF renderer sent an answer that did not decode: {error}"))
    }
}

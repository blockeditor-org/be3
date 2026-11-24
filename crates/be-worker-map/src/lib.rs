//! What the map app and its workers say to each other (guides/adding_an_app.md, Workers). Its one
//! worker, the tile rasterizer (crates/be-worker-map-tiles), decodes a Mapbox vector tile and
//! rasterizes it, off the map's own thread: the app sends it a `Job` for each tile it wants and
//! gets back a `Rasterized` for each, in order. The app depends on this crate alone, so it links
//! no rasterizer.

use serde::{Deserialize, Serialize};

/// The rasterizer's module, as the map's manifest names it under `workers`.
pub const MODULE: &str = "tiles.wasm";

/// The width and height of a rasterized tile, in pixels.
pub const TILE_PIXELS: usize = 512;

/// A tile of the web map pyramid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TileId {
    pub zoom: u8,
    pub x: u32,
    pub y: u32,
}

impl TileId {
    pub fn parent(self) -> Option<TileId> {
        self.zoom.checked_sub(1).map(|zoom| TileId {
            zoom,
            x: self.x / 2,
            y: self.y / 2,
        })
    }

    /// The source's tile this one is drawn from: itself, or its ancestor at `max_zoom`, the
    /// deepest zoom the tile source serves.
    pub fn source(self, max_zoom: u8) -> TileId {
        let mut source = self;
        while source.zoom > max_zoom {
            source = source
                .parent()
                .expect("a tile deeper than the source has a parent");
        }
        source
    }
}

/// What the app asks: rasterize `id` from `body`, the tile `source` as the tile source served it.
#[derive(Serialize, Deserialize)]
pub struct Job {
    pub id: TileId,
    pub source: TileId,
    #[serde(with = "serde_bytes")]
    pub body: Vec<u8>,
}

/// A label the app draws over a tile, in the tile's pixels.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TileLabel {
    pub text: String,
    pub position: [f32; 2],
    pub font_size: f32,
    pub color: [u8; 3],
}

/// A rasterized tile: its RGBA pixels, `TILE_PIXELS` square, and its labels.
#[derive(Serialize, Deserialize)]
pub struct TileRaster {
    #[serde(with = "serde_bytes")]
    pub pixels: Vec<u8>,
    pub labels: Vec<TileLabel>,
}

/// The answer to one `Job`: the tile, or why it could not be drawn.
#[derive(Serialize, Deserialize)]
pub struct Rasterized {
    pub id: TileId,
    pub result: Result<TileRaster, String>,
}

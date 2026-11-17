//! The map's tile rasterizer (crates/be-worker-map says what it is sent and answers): a worker
//! that decodes each Mapbox vector tile it is sent and rasterizes it. `worker.rs` exports `Tiles`
//! as the module `tiles.wasm`; only it and the map's tests, which serve `Tiles` in their own
//! instance, depend on this crate, so the app itself links no rasterizer.

use std::collections::VecDeque;
use std::io::Read;
use std::rc::Rc;

use be_sdk_worker::{App, Work};

mod mvt;
mod raster;

use be_worker_map::{Job, Rasterized, TileId};
use raster::Window;

const MAX_TILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_DECODED: usize = 4;

/// Where tile `id` lies within its `source`, an ancestor of it or itself.
fn window(id: TileId, source: TileId) -> Window {
    let factor = 1u32 << (id.zoom - source.zoom);
    Window {
        factor: factor as f32,
        offset: [
            (id.x - source.x * factor) as f32,
            (id.y - source.y * factor) as f32,
        ],
    }
}

/// The worker: it keeps the last few tiles it decoded, so the deeper tiles cut from one source
/// tile decode it once.
pub struct Tiles {
    decoded: VecDeque<(TileId, Rc<Result<mvt::Tile, String>>)>,
}

impl Work for Tiles {
    fn start(_app: &mut App) -> Result<Self, String> {
        Ok(Self {
            decoded: VecDeque::new(),
        })
    }

    fn receive(&mut self, message: Vec<u8>, app: &mut App) -> Result<(), String> {
        let Job {
            requester,
            id,
            source,
            body,
        } = Job::decode(&message)?;
        let tile = self.decode(source, &body);
        let result = match tile.as_ref() {
            Ok(tile) => Ok(raster::rasterize(tile, id.zoom, window(id, source))),
            Err(error) => Err(error.clone()),
        };
        let answer = Rasterized {
            requester,
            id,
            result,
        };
        app.send(answer.encode());
        Ok(())
    }
}

impl Tiles {
    fn decode(&mut self, source: TileId, body: &[u8]) -> Rc<Result<mvt::Tile, String>> {
        if let Some((_, tile)) = self.decoded.iter().find(|(held, _)| *held == source) {
            return Rc::clone(tile);
        }
        let tile = Rc::new(decode(body));
        self.decoded.push_back((source, Rc::clone(&tile)));
        if self.decoded.len() > MAX_DECODED {
            self.decoded.pop_front();
        }
        tile
    }
}

fn decode(body: &[u8]) -> Result<mvt::Tile, String> {
    if body.starts_with(&[0x1f, 0x8b]) {
        let mut decompressed = Vec::new();
        flate2::read::GzDecoder::new(body)
            .take(MAX_TILE_BYTES)
            .read_to_end(&mut decompressed)
            .map_err(|error| format!("tile decompression failed: {error}"))?;
        return mvt::decode(&decompressed);
    }
    mvt::decode(body)
}

#[cfg(test)]
mod tests;

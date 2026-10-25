use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Read;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use block_editor_beui::{EditorHost, Task, Waker};

use crate::raster::Window;
use crate::{mvt, raster};

pub(crate) const SOURCE_MAX_ZOOM: u8 = 14;
pub(crate) const MAX_TILE_ZOOM: u8 = 17;
const TILE_URL_BASE: &str = "https://vector.openstreetmap.org/shortbread_v1";
const MAX_TILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_IN_FLIGHT: usize = 8;
const MAX_BODIES: usize = 48;
const MAX_DECODED: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct TileId {
    pub zoom: u8,
    pub x: u32,
    pub y: u32,
}

impl TileId {
    pub(crate) fn parent(self) -> Option<TileId> {
        self.zoom.checked_sub(1).map(|zoom| TileId {
            zoom,
            x: self.x / 2,
            y: self.y / 2,
        })
    }

    pub(crate) fn source(self) -> TileId {
        let mut source = self;
        while source.zoom > SOURCE_MAX_ZOOM {
            source = source
                .parent()
                .expect("a tile deeper than the source has a parent");
        }
        source
    }

    fn window(self, source: TileId) -> Window {
        let factor = 1u32 << (self.zoom - source.zoom);
        Window {
            factor: factor as f32,
            offset: [
                (self.x - source.x * factor) as f32,
                (self.y - source.y * factor) as f32,
            ],
        }
    }

    fn url(self) -> String {
        format!("{TILE_URL_BASE}/{}/{}/{}.mvt", self.zoom, self.x, self.y)
    }
}

pub(crate) struct TileResult {
    pub id: TileId,
    pub result: Result<raster::TileRaster, String>,
}

struct Work {
    id: TileId,
    source: TileId,
    body: Arc<Vec<u8>>,
}

type Fetched = Rc<RefCell<Vec<(TileId, Result<Vec<u8>, String>)>>>;

pub(crate) struct TileWorker {
    downloads: HashMap<TileId, Task>,
    fetched: Fetched,
    waker: Waker,
    queued: Vec<TileId>,
    waiting: HashMap<TileId, Vec<TileId>>,
    bodies: HashMap<TileId, Arc<Vec<u8>>>,
    kept: VecDeque<TileId>,
    sent: HashSet<TileId>,
    dropped: Arc<Mutex<HashSet<TileId>>>,
    rasterizing: Sender<Work>,
    rasterized: Receiver<TileResult>,
}

impl TileWorker {
    pub(crate) fn spawn(waker: Waker) -> Self {
        let (rasterizing, work) = channel();
        let (results, rasterized) = channel();
        let dropped = Arc::new(Mutex::new(HashSet::new()));
        let skipped = Arc::clone(&dropped);
        let woken = waker.clone();
        let _ = std::thread::Builder::new()
            .name("map-tile-rasterizer".into())
            .spawn(move || rasterize(work, results, skipped, woken));
        Self {
            downloads: HashMap::new(),
            fetched: Rc::default(),
            waker,
            queued: Vec::new(),
            waiting: HashMap::new(),
            bodies: HashMap::new(),
            kept: VecDeque::new(),
            sent: HashSet::new(),
            dropped,
            rasterizing,
            rasterized,
        }
    }

    pub(crate) fn request(&mut self, id: TileId) {
        let source = id.source();
        if let Some(body) = self.bodies.get(&source).cloned() {
            self.send(id, source, body);
            return;
        }
        let waiting = self.waiting.entry(source).or_default();
        if waiting.contains(&id) {
            return;
        }
        waiting.push(id);
        if waiting.len() == 1 && !self.downloads.contains_key(&source) {
            self.queued.push(source);
        }
    }

    pub(crate) fn cancel(&mut self, id: TileId) -> bool {
        if self.sent.remove(&id) {
            if let Ok(mut dropped) = self.dropped.lock() {
                dropped.insert(id);
            }
            return true;
        }
        let source = id.source();
        let Some(waiting) = self.waiting.get_mut(&source) else {
            return false;
        };
        let before = waiting.len();
        waiting.retain(|other| *other != id);
        let cancelled = waiting.len() != before;
        if waiting.is_empty() {
            self.waiting.remove(&source);
            self.queued.retain(|queued| *queued != source);
        }
        cancelled
    }

    fn send(&mut self, id: TileId, source: TileId, body: Arc<Vec<u8>>) {
        if let Ok(mut dropped) = self.dropped.lock() {
            dropped.remove(&id);
        }
        self.sent.insert(id);
        let _ = self.rasterizing.send(Work { id, source, body });
    }

    fn keep(&mut self, source: TileId, body: Arc<Vec<u8>>) {
        if self.bodies.insert(source, body).is_none() {
            self.kept.push_back(source);
        }
        while self.kept.len() > MAX_BODIES {
            if let Some(oldest) = self.kept.pop_front() {
                self.bodies.remove(&oldest);
            }
        }
    }

    pub(crate) fn dispatch(&mut self, host: &EditorHost) {
        while self.downloads.len() < MAX_IN_FLIGHT {
            let Some(id) = self.queued.pop() else {
                break;
            };
            let fetched = Rc::clone(&self.fetched);
            let waker = self.waker.clone();
            let task = host.then(host.fetch(id.url()), move |result| {
                fetched
                    .borrow_mut()
                    .push((id, result.map_err(|failure| failure.to_string())));
                waker.wake();
            });
            self.downloads.insert(id, task);
        }
    }

    pub(crate) fn forget(self) {
        for task in self.downloads.into_values() {
            task.cancel();
        }
    }

    pub(crate) fn poll(&mut self, host: &EditorHost) -> Vec<TileResult> {
        self.dispatch(host);
        let mut failures = Vec::new();
        let fetched = std::mem::take(&mut *self.fetched.borrow_mut());
        for (source, result) in fetched {
            if self.downloads.remove(&source).is_none() {
                continue;
            }
            let waiting = self.waiting.remove(&source).unwrap_or_default();
            match result {
                Ok(body) => {
                    let body = Arc::new(body);
                    self.keep(source, Arc::clone(&body));
                    for id in waiting {
                        self.send(id, source, Arc::clone(&body));
                    }
                }
                Err(error) => {
                    failures.extend(waiting.into_iter().map(|id| TileResult {
                        id,
                        result: Err(error.clone()),
                    }));
                }
            }
        }
        self.dispatch(host);
        while let Ok(result) = self.rasterized.try_recv() {
            self.sent.remove(&result.id);
            failures.push(result);
        }
        failures
    }
}

fn rasterize(
    work: Receiver<Work>,
    results: Sender<TileResult>,
    dropped: Arc<Mutex<HashSet<TileId>>>,
    waker: Waker,
) {
    let mut decoded: VecDeque<(TileId, Arc<Result<mvt::Tile, String>>)> = VecDeque::new();
    while let Ok(Work { id, source, body }) = work.recv() {
        if dropped.lock().is_ok_and(|mut dropped| dropped.remove(&id)) {
            continue;
        }
        let tile = match decoded.iter().find(|(held, _)| *held == source) {
            Some((_, tile)) => Arc::clone(tile),
            None => {
                let tile = Arc::new(decode(&body));
                decoded.push_back((source, Arc::clone(&tile)));
                if decoded.len() > MAX_DECODED {
                    decoded.pop_front();
                }
                tile
            }
        };
        let result = match tile.as_ref() {
            Ok(tile) => Ok(raster::rasterize(tile, id.zoom, id.window(source))),
            Err(error) => Err(error.clone()),
        };
        if results.send(TileResult { id, result }).is_err() {
            return;
        }
        waker.wake();
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

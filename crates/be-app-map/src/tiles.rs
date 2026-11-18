use std::cell::RefCell;
use std::collections::{HashMap, VecDeque, hash_map::Entry};
use std::rc::Rc;

use be_sdk_beui::network::{Request, Response};
use be_sdk_beui::worker::{Answer, Generation, Shared};
use be_sdk_beui::{EditorHost, Task, Waker};
pub(crate) use be_worker_map::TileId;
use be_worker_map::{Job, Rasterized, TileRaster};
use serde::Deserialize;

pub(crate) const MAX_TILE_ZOOM: u8 = 17;
/// OpenFreeMap's TileJSON, which names where its current tiles are and the deepest zoom they go:
/// their address carries the date they were built and changes every week or so, so the map reads
/// it rather than keeping one. The tiles follow the OpenMapTiles schema, which
/// be-worker-map-tiles draws.
pub(crate) const TILE_JSON: &str = "https://tiles.openfreemap.org/planet";
/// Where the TileJSON may send the map for tiles.
const TILE_HOST: &str = "https://tiles.openfreemap.org/";
const MAX_IN_FLIGHT: usize = 8;
const MAX_BODIES: usize = 48;
/// The most jobs the rasterizer holds at once: one it works on and one queued behind it, so it
/// never waits on the app between tiles, while the app keeps the rest and can still cancel them.
const MAX_RASTERIZING: usize = 2;

/// The part of a TileJSON the map reads.
#[derive(Deserialize)]
struct TileJson {
    /// Tile URL templates, holding `{z}`, `{x}` and `{y}`; the map uses the first.
    tiles: Vec<String>,
    /// The deepest zoom it serves; TileJSON 3.0 makes it optional, defaulting to 30.
    #[serde(default = "default_max_zoom")]
    maxzoom: u8,
}

fn default_max_zoom() -> u8 {
    30
}

/// The tile source, as its TileJSON describes it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TileSource {
    /// The tiles' URL template, holding `{z}`, `{x}` and `{y}`.
    pub template: String,
    /// The deepest zoom it serves: deeper tiles are cut from one of its tiles.
    pub max_zoom: u8,
}

impl TileSource {
    fn from_tile_json(tile_json: TileJson) -> Result<Self, String> {
        let template = tile_json
            .tiles
            .into_iter()
            .next()
            .ok_or_else(|| "the tile source names no tiles".to_owned())?;
        let complete = ["{z}", "{x}", "{y}"]
            .iter()
            .all(|part| template.contains(part));
        if !template.starts_with(TILE_HOST) || !complete {
            return Err("the tile source names tiles elsewhere".to_owned());
        }
        Ok(Self {
            template,
            max_zoom: tile_json.maxzoom,
        })
    }

    fn url(&self, id: TileId) -> String {
        self.template
            .replace("{z}", &id.zoom.to_string())
            .replace("{x}", &id.x.to_string())
            .replace("{y}", &id.y.to_string())
    }
}

/// The tile source, as far as the map has read its TileJSON.
enum Lookup {
    /// Not asked yet, or the last answer failed: the next tile asks again.
    Unread,
    Reading(Task),
    Read(TileSource),
}

type LookupAnswer = Rc<RefCell<Option<Result<TileSource, String>>>>;

pub(crate) struct TileResult {
    pub id: TileId,
    pub result: Result<TileRaster, String>,
}

type Fetched = Rc<RefCell<Vec<(TileId, Result<Vec<u8>, String>)>>>;

pub(crate) struct TileWorker {
    /// Read once per worker: reloading the map starts a new worker, which reads it again.
    lookup: Lookup,
    lookup_answer: LookupAnswer,
    /// Tiles asked for before the tile source was read, which says what each is cut from.
    unsourced: Vec<TileId>,
    downloads: HashMap<TileId, Task>,
    fetched: Fetched,
    waker: Waker,
    queued: Vec<TileId>,
    waiting: HashMap<TileId, Vec<TileId>>,
    bodies: HashMap<TileId, Rc<Vec<u8>>>,
    kept: VecDeque<TileId>,
    /// Tiles to rasterize, oldest first, not yet sent to the rasterizer.
    pending: VecDeque<(TileId, TileId, Rc<Vec<u8>>)>,
    /// Tiles to rasterize that were not cancelled, pending or sent, and the generation of the
    /// rasterizer each pending one went to once it went: what it answers for any other tile, one
    /// cancelled meanwhile, is dropped.
    sent: HashMap<TileId, Option<Generation>>,
    /// How many jobs each generation of the rasterizer was sent and has not answered, cancelled
    /// or not; a generation that stopped answers none of them.
    by_generation: HashMap<Generation, usize>,
    /// Their sum: the jobs the rasterizer has in hand.
    rasterizing: usize,
    /// This map's way to the rasterizer (be-worker-map-tiles), which every map of the app shares,
    /// once it sent it a tile.
    rasterizer: Option<Shared>,
}

impl TileWorker {
    pub(crate) fn spawn(waker: Waker) -> Self {
        Self {
            lookup: Lookup::Unread,
            lookup_answer: Rc::default(),
            unsourced: Vec::new(),
            downloads: HashMap::new(),
            fetched: Rc::default(),
            waker,
            queued: Vec::new(),
            waiting: HashMap::new(),
            bodies: HashMap::new(),
            kept: VecDeque::new(),
            pending: VecDeque::new(),
            sent: HashMap::new(),
            by_generation: HashMap::new(),
            rasterizing: 0,
            rasterizer: None,
        }
    }

    pub(crate) fn request(&mut self, id: TileId) {
        let Lookup::Read(tile_source) = &self.lookup else {
            if !self.unsourced.contains(&id) {
                self.unsourced.push(id);
            }
            return;
        };
        let source = id.source(tile_source.max_zoom);
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
        if self.sent.remove(&id).is_some() {
            self.pending.retain(|(pending, _, _)| *pending != id);
            return true;
        }
        if let Some(index) = self.unsourced.iter().position(|other| *other == id) {
            self.unsourced.remove(index);
            return true;
        }
        let Lookup::Read(tile_source) = &self.lookup else {
            return false;
        };
        let source = id.source(tile_source.max_zoom);
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

    /// Takes the tile source the TileJSON describes, and asks it for the tiles waiting on it.
    fn read(&mut self, tile_source: TileSource) {
        self.lookup = Lookup::Read(tile_source);
        for id in std::mem::take(&mut self.unsourced) {
            self.request(id);
        }
    }

    fn send(&mut self, id: TileId, source: TileId, body: Rc<Vec<u8>>) {
        if let Entry::Vacant(entry) = self.sent.entry(id) {
            entry.insert(None);
            self.pending.push_back((id, source, body));
        }
    }

    fn keep(&mut self, source: TileId, body: Rc<Vec<u8>>) {
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
        let tile_source = match &self.lookup {
            Lookup::Read(tile_source) => Some(tile_source.clone()),
            Lookup::Reading(_) => None,
            Lookup::Unread if self.unsourced.is_empty() => None,
            Lookup::Unread => {
                let answer = Rc::clone(&self.lookup_answer);
                let waker = self.waker.clone();
                let tile_json = host
                    .fetch(Request::get(TILE_JSON))
                    .map(|answer| answer.and_then(Response::success))
                    .and_then(Response::json::<TileJson>);
                let task = host.then(tile_json, move |result| {
                    let tile_source = result
                        .map_err(|failure| format!("the tile source could not be read: {failure}"))
                        .and_then(TileSource::from_tile_json);
                    *answer.borrow_mut() = Some(tile_source);
                    waker.wake();
                });
                self.lookup = Lookup::Reading(task);
                None
            }
        };
        while let Some(tile_source) = &tile_source
            && self.downloads.len() < MAX_IN_FLIGHT
        {
            let Some(id) = self.queued.pop() else {
                break;
            };
            let fetched = Rc::clone(&self.fetched);
            let waker = self.waker.clone();
            let tile = host
                .fetch(Request::get(tile_source.url(id)))
                .map(|answer| answer.and_then(Response::success))
                .and_then(Response::bytes);
            let task = host.then(tile, move |result| {
                fetched
                    .borrow_mut()
                    .push((id, result.map_err(|failure| failure.to_string())));
                waker.wake();
            });
            self.downloads.insert(id, task);
        }
        while self.rasterizing < MAX_RASTERIZING {
            let Some((id, source, body)) = self.pending.pop_front() else {
                break;
            };
            let job = Job {
                id,
                source,
                body: body.as_ref().clone(),
            };
            let waker = self.waker.clone();
            let generation = self
                .rasterizer
                .get_or_insert_with(|| Shared::new(be_worker_map::MODULE, move || waker.wake()))
                .send(&job.encode());
            *self.by_generation.entry(generation).or_default() += 1;
            self.rasterizing += 1;
            if let Some(sent) = self.sent.get_mut(&id) {
                *sent = Some(generation);
            }
        }
    }

    pub(crate) fn forget(mut self) {
        if let Lookup::Reading(task) = std::mem::replace(&mut self.lookup, Lookup::Unread) {
            task.cancel();
        }
        for task in std::mem::take(&mut self.downloads).into_values() {
            task.cancel();
        }
    }

    pub(crate) fn poll(&mut self, host: &EditorHost) -> Vec<TileResult> {
        let mut results = Vec::new();
        let answer = self.lookup_answer.borrow_mut().take();
        match answer {
            Some(Ok(tile_source)) => self.read(tile_source),
            // Every tile asked for meanwhile fails with it; the next asks again.
            Some(Err(error)) => {
                self.lookup = Lookup::Unread;
                results.extend(self.unsourced.drain(..).map(|id| TileResult {
                    id,
                    result: Err(error.clone()),
                }));
            }
            None => {}
        }
        let fetched = std::mem::take(&mut *self.fetched.borrow_mut());
        for (source, result) in fetched {
            if self.downloads.remove(&source).is_none() {
                continue;
            }
            let waiting = self.waiting.remove(&source).unwrap_or_default();
            match result {
                Ok(body) => {
                    let body = Rc::new(body);
                    self.keep(source, Rc::clone(&body));
                    for id in waiting {
                        self.send(id, source, Rc::clone(&body));
                    }
                }
                Err(error) => {
                    results.extend(waiting.into_iter().map(|id| TileResult {
                        id,
                        result: Err(error.clone()),
                    }));
                }
            }
        }
        while let Some(Answer { generation, result }) =
            self.rasterizer.as_ref().and_then(Shared::try_receive)
        {
            match result.and_then(|message| Rasterized::decode(&message)) {
                Ok(Rasterized { id, result }) => {
                    if let Some(count) = self.by_generation.get_mut(&generation)
                        && *count > 0
                    {
                        *count -= 1;
                        self.rasterizing -= 1;
                    }
                    if self.sent.remove(&id).is_some() {
                        results.push(TileResult { id, result });
                    }
                }
                // That generation of the rasterizer stopped: what was sent to it fails, and the
                // next tile starts it anew. A tile still pending, or sent to a later generation
                // (dispatch can restart it before this hears the stop), waits for that.
                Err(why) => {
                    let lost = self.by_generation.remove(&generation).unwrap_or(0);
                    self.rasterizing -= lost;
                    let failed: Vec<TileId> = self
                        .sent
                        .iter()
                        .filter(|(_, sent)| **sent == Some(generation))
                        .map(|(id, _)| *id)
                        .collect();
                    for id in failed {
                        self.sent.remove(&id);
                        results.push(TileResult {
                            id,
                            result: Err(why.clone()),
                        });
                    }
                }
            }
        }
        self.dispatch(host);
        results
    }
}

#[cfg(test)]
mod tests;

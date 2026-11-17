use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use be_sdk_beui::worker::Shared;
use be_sdk_beui::network::{Request, Response};
use be_sdk_beui::{EditorHost, Task, Waker};
pub(crate) use be_worker_map::TileId;
use be_worker_map::{Job, Rasterized, TileRaster};

pub(crate) const MAX_TILE_ZOOM: u8 = 17;
const TILE_URL_BASE: &str = "https://vector.openstreetmap.org/shortbread_v1";
const MAX_IN_FLIGHT: usize = 8;
const MAX_BODIES: usize = 48;
/// The most jobs the rasterizer holds at once: one it works on and one queued behind it, so it
/// never waits on the app between tiles, while the app keeps the rest and can still cancel them.
const MAX_RASTERIZING: usize = 2;

fn url(id: TileId) -> String {
    format!("{TILE_URL_BASE}/{}/{}/{}.mvt", id.zoom, id.x, id.y)
}

pub(crate) struct TileResult {
    pub id: TileId,
    pub result: Result<TileRaster, String>,
}

type Fetched = Rc<RefCell<Vec<(TileId, Result<Vec<u8>, String>)>>>;

pub(crate) struct TileWorker {
    downloads: HashMap<TileId, Task>,
    fetched: Fetched,
    waker: Waker,
    queued: Vec<TileId>,
    waiting: HashMap<TileId, Vec<TileId>>,
    bodies: HashMap<TileId, Rc<Vec<u8>>>,
    kept: VecDeque<TileId>,
    /// Tiles to rasterize, oldest first, not yet sent to the rasterizer.
    pending: VecDeque<(TileId, TileId, Rc<Vec<u8>>)>,
    /// Tiles the rasterizer was sent and has not answered: what it answers for any other tile, one
    /// cancelled meanwhile, is dropped.
    sent: HashSet<TileId>,
    rasterizing: usize,
    /// This map's way to the rasterizer (be-worker-map-tiles), which every map of the app shares,
    /// once it sent it a tile.
    rasterizer: Option<Shared>,
}

impl TileWorker {
    pub(crate) fn spawn(waker: Waker) -> Self {
        Self {
            downloads: HashMap::new(),
            fetched: Rc::default(),
            waker,
            queued: Vec::new(),
            waiting: HashMap::new(),
            bodies: HashMap::new(),
            kept: VecDeque::new(),
            pending: VecDeque::new(),
            sent: HashSet::new(),
            rasterizing: 0,
            rasterizer: None,
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
            self.pending.retain(|(pending, _, _)| *pending != id);
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

    fn send(&mut self, id: TileId, source: TileId, body: Rc<Vec<u8>>) {
        if self.sent.insert(id) {
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
        while self.downloads.len() < MAX_IN_FLIGHT {
            let Some(id) = self.queued.pop() else {
                break;
            };
            let fetched = Rc::clone(&self.fetched);
            let waker = self.waker.clone();
            let tile = host
                .fetch(Request::get(url(id)))
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
            self.rasterizer
                .get_or_insert_with(|| {
                    Shared::new(be_worker_map::MODULE, move || waker.wake())
                })
                .send(&job.encode());
            self.rasterizing += 1;
        }
    }

    pub(crate) fn forget(mut self) {
        for task in std::mem::take(&mut self.downloads).into_values() {
            task.cancel();
        }
    }

    pub(crate) fn poll(&mut self, host: &EditorHost) -> Vec<TileResult> {
        let mut results = Vec::new();
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
        while let Some(answer) = self.rasterizer.as_ref().and_then(Shared::try_receive) {
            match answer.and_then(|message| Rasterized::decode(&message)) {
                Ok(Rasterized { id, result }) => {
                    self.rasterizing = self.rasterizing.saturating_sub(1);
                    if self.sent.remove(&id) {
                        results.push(TileResult { id, result });
                    }
                }
                // The rasterizer stopped: what it was sent fails, and the next tile starts it
                // anew. A tile still pending was never sent, so it waits for that.
                Err(why) => {
                    self.rasterizing = 0;
                    let pending: HashSet<TileId> =
                        self.pending.iter().map(|(id, _, _)| *id).collect();
                    let failed: Vec<TileId> = self
                        .sent
                        .iter()
                        .filter(|id| !pending.contains(id))
                        .copied()
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

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::{Rc, Weak};

use be_sdk_beui::worker::{Sender, Worker};
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

/// What the rasterizer answered one map since it last looked: a tile, or why it stopped.
type Answered = Rc<RefCell<VecDeque<Result<Rasterized, String>>>>;

/// Each map's answers, by its `requester`, and the waker that tells it they came.
type Routes = Rc<RefCell<HashMap<u64, (Answered, Waker)>>>;

thread_local! {
    /// The rasterizer every map of the app shares while any holds it.
    static RASTERIZER: RefCell<Weak<Rasterizer>> = RefCell::new(Weak::new());
}

/// The rasterizer (be-worker-map-tiles), a worker of the map's own that every map it shows
/// shares, rather than one each, and the task that hears it and hands each map its own answers.
/// Dropping it, once no map holds it, stops both.
struct Rasterizer {
    sender: Sender,
    routes: Routes,
    next: Cell<u64>,
    /// Whether it stopped: a map that asks for the shared rasterizer then gets a new one.
    stopped: Rc<Cell<bool>>,
    hearing: Task,
}

impl Drop for Rasterizer {
    fn drop(&mut self) {
        self.hearing.cancel();
    }
}

impl Rasterizer {
    fn shared() -> Rc<Self> {
        RASTERIZER.with(|shared| {
            if let Some(rasterizer) = shared.borrow().upgrade()
                && !rasterizer.stopped.get()
            {
                return rasterizer;
            }
            let rasterizer = Rc::new(Self::spawn());
            *shared.borrow_mut() = Rc::downgrade(&rasterizer);
            rasterizer
        })
    }

    fn spawn() -> Self {
        let mut worker = Worker::spawn(be_worker_map::MODULE);
        let sender = worker.sender();
        let routes = Routes::default();
        let heard = Rc::clone(&routes);
        let stopped = Rc::new(Cell::new(false));
        let stops = Rc::clone(&stopped);
        let hearing = be_sdk_beui::spawn(async move {
            loop {
                let answer = worker
                    .receive()
                    .await
                    .and_then(|message| Rasterized::decode(&message));
                let routes = heard.borrow();
                match answer {
                    Ok(answer) => {
                        if let Some((answered, waker)) = routes.get(&answer.requester) {
                            answered.borrow_mut().push_back(Ok(answer));
                            waker.wake();
                        }
                    }
                    Err(why) => {
                        stops.set(true);
                        for (answered, waker) in routes.values() {
                            answered.borrow_mut().push_back(Err(why.clone()));
                            waker.wake();
                        }
                        return;
                    }
                }
            }
        });
        Self {
            sender,
            routes,
            next: Cell::new(0),
            stopped,
            hearing,
        }
    }

    /// A requester for one map, whose answers go to `answered` and wake it with `waker`.
    fn route(&self, answered: &Answered, waker: &Waker) -> u64 {
        let requester = self.next.get();
        self.next.set(requester + 1);
        self.routes
            .borrow_mut()
            .insert(requester, (Rc::clone(answered), waker.clone()));
        requester
    }
}

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
    /// The shared rasterizer and this map's requester, once it sent one a tile.
    rasterizer: Option<(Rc<Rasterizer>, u64)>,
    answered: Answered,
    /// Why the rasterizer stopped, once it did: every tile after fails with it.
    stopped: Option<String>,
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
            answered: Rc::default(),
            stopped: None,
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
        if self.stopped.is_some() {
            return;
        }
        while self.rasterizing < MAX_RASTERIZING {
            let Some((id, source, body)) = self.pending.pop_front() else {
                break;
            };
            let (rasterizer, requester) = self.rasterizer.get_or_insert_with(|| {
                let rasterizer = Rasterizer::shared();
                let requester = rasterizer.route(&self.answered, &self.waker);
                (rasterizer, requester)
            });
            let job = Job {
                requester: *requester,
                id,
                source,
                body: body.as_ref().clone(),
            };
            rasterizer.sender.send(job.encode());
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
        let answered = std::mem::take(&mut *self.answered.borrow_mut());
        for answer in answered {
            match answer {
                Ok(Rasterized { id, result, .. }) => {
                    self.rasterizing = self.rasterizing.saturating_sub(1);
                    if self.sent.remove(&id) {
                        results.push(TileResult { id, result });
                    }
                }
                Err(why) => self.stopped = Some(why),
            }
        }
        if let Some(why) = &self.stopped {
            self.pending.clear();
            results.extend(self.sent.drain().map(|id| TileResult {
                id,
                result: Err(why.clone()),
            }));
        }
        self.dispatch(host);
        results
    }
}

impl Drop for TileWorker {
    fn drop(&mut self) {
        if let Some((rasterizer, requester)) = &self.rasterizer {
            rasterizer.routes.borrow_mut().remove(requester);
        }
    }
}

#[cfg(test)]
mod tests;

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
    rc::{Rc, Weak},
    time::Instant,
};

use be_sdk_beui::worker::{Sender, Worker};
use be_sdk_beui::{
    PerformanceReporter, Task, Waker,
    beui::{Image, Pos2, Vec2},
};
pub(crate) use be_worker_pdf::{DETAIL_MAX_DIM, MIN_SCALE, Target as RenderTarget};
use be_worker_pdf::{Answer, Job, Measure, Rendered};

pub(crate) struct RenderedTile {
    pub(crate) page_count: usize,
    pub(crate) page_index: usize,
    pub(crate) page_size_pts: Vec2,
    pub(crate) scale: f32,
    pub(crate) origin_pts: Pos2,
    pub(crate) size_pts: Vec2,
    pub(crate) image: Image,
}

#[derive(Clone, Copy)]
pub(crate) struct RenderRequest {
    pub(crate) revision: u64,
    pub(crate) page: usize,
    pub(crate) target: RenderTarget,
}

pub(crate) type RenderJobResult = Result<RenderedTile, String>;

/// What the renderer answered one pane since it last looked.
type Answered = Rc<RefCell<VecDeque<Result<Answer, String>>>>;

/// Each pane's answers, by its `requester`, and the waker that tells it they came.
type Routes = Rc<RefCell<HashMap<u64, (Answered, Waker)>>>;

thread_local! {
    /// The renderer every PDF the app shows shares while any holds it.
    static RENDERER: RefCell<Weak<Renderer>> = RefCell::new(Weak::new());
}

/// The renderer (be-worker-pdf), a worker of the PDF app's own that every page it shows shares,
/// rather than one each, and the task that hears it and hands each pane its own answers.
/// Dropping it, once no pane holds it, stops both.
struct Renderer {
    sender: Sender,
    routes: Routes,
    next: Cell<u64>,
    /// Whether it stopped: a pane that asks for the shared renderer then gets a new one.
    stopped: Rc<Cell<bool>>,
    hearing: Task,
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.hearing.cancel();
    }
}

impl Renderer {
    fn shared() -> Rc<Self> {
        RENDERER.with(|shared| {
            if let Some(renderer) = shared.borrow().upgrade()
                && !renderer.stopped.get()
            {
                return renderer;
            }
            let renderer = Rc::new(Self::spawn());
            *shared.borrow_mut() = Rc::downgrade(&renderer);
            renderer
        })
    }

    fn spawn() -> Self {
        let mut worker = Worker::spawn(be_worker_pdf::MODULE);
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
                    .and_then(|message| Answer::decode(&message));
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
}

/// One pane's way to the shared renderer: it sends the pane's jobs and hears only their answers,
/// in order.
pub(crate) struct RenderClient {
    renderer: Rc<Renderer>,
    requester: u64,
    answered: Answered,
    /// When each job still unanswered was sent, oldest first.
    sent: VecDeque<Instant>,
}

impl RenderClient {
    pub(crate) fn new(waker: &Waker) -> Self {
        let renderer = Renderer::shared();
        let requester = renderer.next.get();
        renderer.next.set(requester + 1);
        let answered = Answered::default();
        renderer
            .routes
            .borrow_mut()
            .insert(requester, (Rc::clone(&answered), waker.clone()));
        Self {
            renderer,
            requester,
            answered,
            sent: VecDeque::new(),
        }
    }

    /// Whether the renderer this client sends to stopped, so a new client is needed.
    pub(crate) fn stopped(&self) -> bool {
        self.renderer.stopped.get()
    }

    pub(crate) fn send(&mut self, data: Vec<u8>, page: usize, target: RenderTarget) {
        let job = Job {
            requester: self.requester,
            page,
            target,
            data,
        };
        self.renderer.sender.send(job.encode());
        self.sent.push_back(Instant::now());
    }

    /// The answer to the oldest job still unanswered, once it came, with what the renderer
    /// measured recorded to `performance`.
    pub(crate) fn receive(&mut self, performance: &PerformanceReporter) -> Option<RenderJobResult> {
        let answer = self.answered.borrow_mut().pop_front()?;
        let sent = self.sent.pop_front();
        let answer = match answer {
            Ok(answer) => answer,
            Err(why) => {
                self.sent.clear();
                return Some(Err(why));
            }
        };
        if let Some(sent) = sent {
            performance.record_duration("Round trip", sent.elapsed());
        }
        for measure in answer.measures {
            match measure {
                Measure::Duration(name, took) => performance.record_duration(name, took),
                Measure::Count(name, count) => performance.record_count(name, count),
            }
        }
        Some(answer.result.map(|rendered| {
            let Rendered {
                page_count,
                page_index,
                page_size_pts,
                scale,
                origin_pts,
                size_pts,
                width,
                height,
                rgba,
            } = rendered;
            RenderedTile {
                page_count,
                page_index,
                page_size_pts,
                scale,
                origin_pts,
                size_pts,
                image: Image::from_rgba(width, height, rgba),
            }
        }))
    }
}

impl Drop for RenderClient {
    fn drop(&mut self) {
        self.renderer.routes.borrow_mut().remove(&self.requester);
    }
}

//! One worker that every editor of an app asks things of, rather than one each: a canvas can show
//! more editors than an app may run workers (guides/adding_an_app.md, Workers).

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
    future::Future,
    rc::{Rc, Weak},
    task::{Poll, Waker},
};

use be_wasm_sandbox_api::ops::guest;

use crate::{Received, Sender, Worker};

/// Which instance of a shared worker a message went to and an answer came from. A worker that
/// stopped is started anew by the next `send`, as the next generation, so an editor can tell what
/// a stop fails (what it sent to that generation) from what it sent since.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Generation(u64);

/// An answer to one client of a shared worker (`Shared::try_receive`): a message the worker sent
/// it, or why the worker stopped, from the generation it came from.
#[derive(Debug, PartialEq, Eq)]
pub struct Answer {
    pub generation: Generation,
    /// The worker's message, or why the generation stopped: nothing more sent to it will be
    /// answered.
    pub result: Result<Vec<u8>, String>,
}

type Answers = Rc<RefCell<VecDeque<Answer>>>;

/// Where a client's answers go, and how it hears they came.
struct Route {
    answers: Answers,
    wake: Rc<dyn Fn()>,
}

type Routes = Rc<RefCell<HashMap<u64, Route>>>;

thread_local! {
    /// The running worker of each module, by the executor whose app spawned it (so one test's app
    /// never finds another's) and the module.
    static RUNNING: RefCell<HashMap<(u64, String), Weak<Running>>> = RefCell::new(HashMap::new());

    /// The generation the next worker started is.
    static NEXT_GENERATION: Cell<u64> = const { Cell::new(1) };
}

/// A worker module's one instance, while any client holds it, and the task that hears it and
/// hands each client its answers.
struct Running {
    generation: Generation,
    sender: Sender,
    routes: Routes,
    next: Cell<u64>,
    /// Whether it stopped, after which a client that sends starts it anew.
    stopped: Rc<Cell<bool>>,
    _hearing: Hearing,
}

impl Running {
    /// The running worker of `module`, started if none runs or the one that ran stopped.
    fn of(module: &str) -> Rc<Self> {
        let key = (guest::executor(), module.to_owned());
        RUNNING.with(|running| {
            let mut running = running.borrow_mut();
            running.retain(|_, worker| worker.strong_count() > 0);
            if let Some(worker) = running.get(&key).and_then(Weak::upgrade)
                && !worker.stopped.get()
            {
                return worker;
            }
            let worker = Rc::new(Self::start(module));
            running.insert(key, Rc::downgrade(&worker));
            worker
        })
    }

    fn start(module: &str) -> Self {
        let generation = Generation(NEXT_GENERATION.replace(NEXT_GENERATION.get() + 1));
        let mut worker = Worker::spawn(module);
        let sender = worker.sender();
        let routes = Routes::default();
        let stopped = Rc::new(Cell::new(false));
        let heard = Rc::clone(&routes);
        let stops = Rc::clone(&stopped);
        let hearing = Hearing::spawn(async move {
            loop {
                let answer = worker.receive_framed().await;
                let routes = heard.borrow();
                let why = match answer {
                    Received::Message(client, message) => {
                        if let Some(route) = routes.get(&client) {
                            route.answers.borrow_mut().push_back(Answer {
                                generation,
                                result: Ok(message),
                            });
                            (route.wake)();
                        }
                        continue;
                    }
                    Received::Stopped(why) => why,
                    // Which client it answers is lost, so it fails every client's, and the
                    // generation is dropped as one that stopped.
                    Received::Unframed => crate::UNFRAMED.to_owned(),
                };
                stops.set(true);
                for route in routes.values() {
                    route.answers.borrow_mut().push_back(Answer {
                        generation,
                        result: Err(why.clone()),
                    });
                    (route.wake)();
                }
                return;
            }
        });
        Self {
            generation,
            sender,
            routes,
            next: Cell::new(1),
            stopped,
            _hearing: hearing,
        }
    }

    /// A new client, whose answers go to `route`.
    fn join(&self, route: Route) -> u64 {
        let client = self.next.get();
        self.next.set(client + 1);
        self.routes.borrow_mut().insert(client, route);
        client
    }
}

/// One editor's way to a worker of the app's that every editor shares (`Shared::new`): it sends
/// that editor's messages and hears only their answers, in order. Every `Shared` of the same
/// module in the app talks to one instance, which stops once none holds it.
pub struct Shared {
    module: String,
    running: Rc<Running>,
    client: u64,
    answers: Answers,
    wake: Rc<dyn Fn()>,
}

impl Shared {
    /// A client of the worker module `module` (such as `"tiles.wasm"`), started if no other
    /// client runs it. `wake` is called whenever an answer comes, which `try_receive` then takes.
    pub fn new(module: &str, wake: impl Fn() + 'static) -> Self {
        let running = Running::of(module);
        let answers = Answers::default();
        let wake: Rc<dyn Fn()> = Rc::new(wake);
        let client = running.join(Route {
            answers: Rc::clone(&answers),
            wake: Rc::clone(&wake),
        });
        Self {
            module: module.to_owned(),
            running,
            client,
            answers,
            wake,
        }
    }

    /// Sends the worker one message, and answers the generation it went to. If the worker
    /// stopped (a job it could not do stops it), this starts it anew first, as the next
    /// generation, so one bad job fails only what was in flight.
    pub fn send(&mut self, message: &[u8]) -> Generation {
        if self.running.stopped.get() {
            self.leave();
            self.running = Running::of(&self.module);
            self.client = self.running.join(Route {
                answers: Rc::clone(&self.answers),
                wake: Rc::clone(&self.wake),
            });
        }
        self.running.sender.send_as(self.client, message);
        self.running.generation
    }

    /// The next answer to this client, once it came. An error says why a generation stopped,
    /// which fails everything this client sent to that generation, and nothing sent to a later
    /// one: none of it will be answered.
    pub fn try_receive(&self) -> Option<Answer> {
        self.answers.borrow_mut().pop_front()
    }

    fn leave(&self) {
        self.running.routes.borrow_mut().remove(&self.client);
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        self.leave();
    }
}

/// The task that hears a running worker, which ends, dropping the worker, once this is dropped.
struct Hearing {
    canceled: Rc<Cell<bool>>,
    waker: Rc<RefCell<Option<Waker>>>,
}

impl Hearing {
    fn spawn(future: impl Future<Output = ()> + 'static) -> Self {
        let canceled = Rc::new(Cell::new(false));
        let waker = Rc::new(RefCell::new(None::<Waker>));
        let (cancels, wakes) = (Rc::clone(&canceled), Rc::clone(&waker));
        let mut future = Box::pin(future);
        guest::spawn(std::future::poll_fn(move |context| {
            if cancels.get() {
                return Poll::Ready(());
            }
            let polled = future.as_mut().poll(context);
            if polled.is_pending() {
                *wakes.borrow_mut() = Some(context.waker().clone());
            }
            polled
        }));
        Self { canceled, waker }
    }
}

impl Drop for Hearing {
    fn drop(&mut self) {
        self.canceled.set(true);
        if let Some(waker) = self.waker.borrow_mut().take() {
            waker.wake();
        }
    }
}

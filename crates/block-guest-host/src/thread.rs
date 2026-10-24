use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::AtomicBool,
        mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError},
    },
    thread,
    time::{Duration, Instant},
};

use crate::{Channel, Event, Presented, Produced, Received, Wake, panic_message};

pub trait Runnable {
    type Presented: Presented + Send + 'static;

    fn start(&mut self) -> Result<Produced<Self::Presented>, String>;

    fn step(&mut self, frames: Vec<Vec<u8>>) -> Result<Produced<Self::Presented>, String>;

    fn take_wake(&mut self) -> bool;

    fn stop(&mut self);
}

pub type Contain = fn(Box<dyn FnOnce() + Send>) -> Result<(), String>;

pub struct Spawn {
    pub name: String,
    pub wake: Wake,
    pub waiting: Arc<AtomicBool>,
    pub interrupt: Box<dyn Fn() + Send>,
    pub contain: Contain,
}

pub fn catch(run: Box<dyn FnOnce() + Send>) -> Result<(), String> {
    catch_unwind(AssertUnwindSafe(run)).map_err(|panic| panic_message(&*panic))
}

thread_local! {
    static STOPPING: RefCell<Vec<Stopping>> = const { RefCell::new(Vec::new()) };
}

struct Stopping {
    handle: thread::JoinHandle<()>,
    ended: Receiver<()>,
}

pub fn wait_for_workers(within: Duration) {
    let stopping = STOPPING.with(|stopping| std::mem::take(&mut *stopping.borrow_mut()));
    let deadline = Instant::now() + within;
    for thread in stopping {
        let left = deadline.saturating_duration_since(Instant::now());
        match thread.ended.recv_timeout(left) {
            Err(RecvTimeoutError::Disconnected) => {
                let _ = thread.handle.join();
            }
            Ok(()) | Err(RecvTimeoutError::Timeout) => {
                eprintln!("a guest's thread did not stop in time, so it is left running");
            }
        }
    }
}

pub struct Threaded<P> {
    steps: Sender<Vec<Vec<u8>>>,
    events: Receiver<Event<P>>,
    interrupt: Box<dyn Fn() + Send>,
    thread: Option<Stopping>,
}

impl<P> Drop for Threaded<P> {
    fn drop(&mut self) {
        (self.interrupt)();
        let Some(thread) = self.thread.take() else {
            return;
        };
        STOPPING.with(|stopping| {
            let mut stopping = stopping.borrow_mut();
            stopping.retain(|stopping| !stopping.handle.is_finished());
            stopping.push(thread);
        });
    }
}

impl<P: Presented> Channel for Threaded<P> {
    type Presented = P;

    fn step(&mut self, frames: Vec<Vec<u8>>) -> bool {
        self.steps.send(frames).is_ok()
    }

    fn receive(&mut self) -> Received<P> {
        match self.events.try_recv() {
            Ok(event) => Received::Event(event),
            Err(TryRecvError::Empty) => Received::Empty,
            Err(TryRecvError::Disconnected) => Received::Stopped,
        }
    }

    fn wait(&mut self, timeout: Duration) -> Option<Received<P>> {
        Some(match self.events.recv_timeout(timeout) {
            Ok(event) => Received::Event(event),
            Err(RecvTimeoutError::Timeout) => Received::Empty,
            Err(RecvTimeoutError::Disconnected) => Received::Stopped,
        })
    }
}

pub fn spawn<R: Runnable + 'static>(
    options: Spawn,
    load: impl FnOnce() -> Result<R, String> + Send + 'static,
) -> Result<Threaded<R::Presented>, String> {
    let Spawn {
        name,
        wake,
        waiting,
        interrupt,
        contain,
    } = options;
    let (steps, orders) = mpsc::channel();
    let (reports, events) = mpsc::channel();
    let (ending, ended) = mpsc::channel();
    let handle = thread::Builder::new()
        .name(name)
        .spawn(move || {
            let _ending: Sender<()> = ending;
            let failed = reports.clone();
            let woken = Arc::clone(&wake);
            let ran = contain(Box::new(move || {
                run(load, orders, reports, wake, waiting);
            }));
            if let Err(panic) = ran {
                let _ = failed.send(Event::Failed(format!(
                    "The app failed while running it: {panic}"
                )));
                woken();
            }
        })
        .map_err(|error| format!("the guest's thread could not start: {error}"))?;
    Ok(Threaded {
        steps,
        events,
        interrupt,
        thread: Some(Stopping { handle, ended }),
    })
}

fn run<R: Runnable>(
    load: impl FnOnce() -> Result<R, String>,
    orders: Receiver<Vec<Vec<u8>>>,
    reports: Sender<Event<R::Presented>>,
    wake: Wake,
    waiting: Arc<AtomicBool>,
) {
    let report = |event: Event<R::Presented>, woken: bool| {
        let wakes = event.wakes(woken, &waiting);
        let sent = reports.send(event).is_ok();
        if wakes {
            wake();
        }
        sent
    };
    let mut guest = match load() {
        Ok(guest) => guest,
        Err(error) => {
            report(Event::Failed(error), true);
            return;
        }
    };
    match guest.start() {
        Ok(produced) => {
            if !report(Event::Ready(produced), true) {
                return;
            }
        }
        Err(error) => {
            report(Event::Failed(error), true);
            return;
        }
    }
    for frames in orders {
        let event = match guest.step(frames) {
            Ok(produced) => Event::Stepped(produced),
            Err(error) => Event::Failed(error),
        };
        let woken = guest.take_wake();
        let failed = matches!(event, Event::Failed(_));
        if !report(event, woken) || failed {
            return;
        }
    }
    guest.stop();
}

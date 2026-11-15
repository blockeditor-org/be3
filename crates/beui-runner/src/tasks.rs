//! Runs the futures of a program's async memos (beui-reactive's spawner): a future is polled
//! at the start of the frame after it was spawned or woken.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake};

use beui_core::program::Waker;
use beui_reactive::LocalFuture;

#[derive(Default)]
pub(crate) struct Tasks {
    futures: RefCell<BTreeMap<u64, LocalFuture>>,
    next: Cell<u64>,
    woken: Arc<Woken>,
}

#[derive(Default)]
struct Woken {
    ids: Mutex<Vec<u64>>,
    frame: Mutex<Option<Waker>>,
}

impl Woken {
    fn push(&self, id: u64) {
        self.ids.lock().expect("woken tasks").push(id);
        let frame = self.frame.lock().expect("woken tasks").clone();
        if let Some(frame) = frame {
            frame.wake();
        }
    }
}

struct Wakes {
    id: u64,
    woken: Arc<Woken>,
}

impl Wake for Wakes {
    fn wake(self: Arc<Self>) {
        self.woken.push(self.id);
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.woken.push(self.id);
    }
}

impl Tasks {
    /// Makes a spawner for this thread.
    pub(crate) fn install() -> Rc<Self> {
        let tasks = Rc::new(Self::default());
        let spawning = Rc::downgrade(&tasks);
        beui_reactive::spawn_with(move |future| match spawning.upgrade() {
            Some(tasks) => {
                tasks.spawn(future);
                Ok(())
            }
            None => Err(future),
        });
        tasks
    }

    /// Asks `frame` for a frame when a future is spawned or woken.
    pub(crate) fn wake_with(&self, frame: Waker) {
        *self.woken.frame.lock().expect("woken tasks") = Some(frame.clone());
        if !self.woken.ids.lock().expect("woken tasks").is_empty() {
            frame.wake();
        }
    }

    fn spawn(&self, future: LocalFuture) {
        let id = self.next.get();
        self.next.set(id + 1);
        self.futures.borrow_mut().insert(id, future);
        self.woken.push(id);
    }

    /// Polls each future woken since the last call once. One woken again while it runs is
    /// polled in the next frame.
    pub(crate) fn poll_woken(&self) {
        let ids = std::mem::take(&mut *self.woken.ids.lock().expect("woken tasks"));
        for id in ids {
            let Some(mut future) = self.futures.borrow_mut().remove(&id) else {
                continue;
            };
            let waker = std::task::Waker::from(Arc::new(Wakes {
                id,
                woken: Arc::clone(&self.woken),
            }));
            if let Poll::Pending = future.as_mut().poll(&mut Context::from_waker(&waker)) {
                self.futures.borrow_mut().insert(id, future);
            }
        }
    }
}

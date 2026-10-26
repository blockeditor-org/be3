use std::{
    future::poll_fn,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    task::Poll,
};

use futures_util::task::AtomicWaker;

#[derive(Default)]
struct State {
    count: AtomicU64,
    closed: AtomicBool,
    waker: AtomicWaker,
}

pub(crate) struct Nudges(Arc<State>);

pub(crate) struct Nudged {
    state: Arc<State>,
    seen: u64,
}

#[derive(Debug)]
pub(crate) struct Stopped;

pub(crate) fn channel() -> (Nudges, Nudged) {
    let state = Arc::new(State::default());
    let nudged = Nudged {
        state: Arc::clone(&state),
        seen: 0,
    };
    (Nudges(state), nudged)
}

impl Nudges {
    pub(crate) fn nudge(&self) {
        self.0.count.fetch_add(1, Ordering::AcqRel);
        self.0.waker.wake();
    }
}

impl Drop for Nudges {
    fn drop(&mut self) {
        self.0.closed.store(true, Ordering::Release);
        self.0.waker.wake();
    }
}

impl Nudged {
    pub(crate) fn mark_unchanged(&mut self) {
        self.seen = self.state.count.load(Ordering::Acquire);
    }

    pub(crate) async fn changed(&mut self) -> Result<(), Stopped> {
        poll_fn(|context| {
            self.state.waker.register(context.waker());
            if self.state.closed.load(Ordering::Acquire) {
                return Poll::Ready(Err(Stopped));
            }
            let count = self.state.count.load(Ordering::Acquire);
            if count != self.seen {
                self.seen = count;
                return Poll::Ready(Ok(()));
            }
            Poll::Pending
        })
        .await
    }
}

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context as TaskContext, Poll, Waker as TaskWaker};

use crate::app::Waker;

mod diff;
mod engine;
pub mod keys;
#[cfg(all(unix, not(target_arch = "wasm32")))]
pub mod socket;
mod target;
mod window;

pub use diff::changes;
pub use engine::{Automation, PANE, Settled, USAGE, View, World};
pub use window::{MAX_PIXELS, Simulation, WindowSize};

pub struct Capture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub enum Reply {
    Text(String),
    Image {
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    },
    Error(String),
}

pub struct Request {
    pub words: Vec<String>,
    pub reply: Box<dyn FnOnce(Reply) + Send>,
    pub cancelled: Arc<AtomicBool>,
}

#[derive(Default)]
struct Answer {
    reply: Option<Reply>,
    answered: bool,
    waker: Option<TaskWaker>,
}

pub struct Answered {
    answer: Arc<Mutex<Answer>>,
    cancelled: Arc<AtomicBool>,
}

#[derive(Clone)]
pub struct Canceller {
    answer: Arc<Mutex<Answer>>,
    cancelled: Arc<AtomicBool>,
}

impl Answered {
    pub fn canceller(&self) -> Canceller {
        Canceller {
            answer: self.answer.clone(),
            cancelled: self.cancelled.clone(),
        }
    }
}

impl Canceller {
    pub fn cancel(&self, why: String) {
        self.cancelled.store(true, Ordering::SeqCst);
        answer(&self.answer, Reply::Error(why));
    }
}

fn answer(answer: &Mutex<Answer>, reply: Reply) {
    let waker = {
        let mut answer = answer.lock().unwrap_or_else(|poison| poison.into_inner());
        if answer.reply.is_some() || answer.answered {
            return;
        }
        answer.answered = true;
        answer.reply = Some(reply);
        answer.waker.take()
    };
    if let Some(waker) = waker {
        waker.wake();
    }
}

impl Future for Answered {
    type Output = Reply;

    fn poll(self: Pin<&mut Self>, context: &mut TaskContext<'_>) -> Poll<Reply> {
        let mut answer = self
            .answer
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        match answer.reply.take() {
            Some(reply) => Poll::Ready(reply),
            None => {
                answer.waker = Some(context.waker().clone());
                Poll::Pending
            }
        }
    }
}

pub fn ask(inbox: &Inbox, words: Vec<String>) -> Answered {
    let answer = Arc::new(Mutex::new(Answer::default()));
    let cancelled = Arc::new(AtomicBool::new(false));
    let answering = answer.clone();
    inbox.send(Request {
        words,
        reply: Box::new(move |reply| self::answer(&answering, reply)),
        cancelled: cancelled.clone(),
    });
    Answered { answer, cancelled }
}

#[derive(Clone, Default)]
pub struct Inbox {
    inner: Arc<Mutex<InboxState>>,
}

#[derive(Default)]
struct InboxState {
    requests: VecDeque<Request>,
    waker: Option<Waker>,
}

impl Inbox {
    pub fn send(&self, request: Request) {
        let waker = {
            let mut state = self
                .inner
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            state.requests.push_back(request);
            state.waker.clone()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }

    pub fn set_waker(&self, waker: Waker) {
        let pending = {
            let mut state = self
                .inner
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            state.waker = Some(waker.clone());
            !state.requests.is_empty()
        };
        if pending {
            waker.wake();
        }
    }

    pub(crate) fn take(&self) -> Option<Request> {
        self.inner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .requests
            .pop_front()
    }
}

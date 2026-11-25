//! The async half of a local interface (guides/type_modules.md, "Local interfaces"): what its
//! async funcs, futures and streams answer arrives through the interface's one generated
//! `[poll]` import, as a list of [`Settled`].
//!
//! The host answers each such call at once with a ticket and settles it later with a
//! [`Promise`] (an async func or a future) or a [`Sender`] (a stream). The guest's call answers
//! a [`Ticket`] or an [`Items`], which the generated `poll` fills from a [`Settling`].

use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    future::Future,
    marker::PhantomData,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};

use serde::{Deserialize, Serialize};

use crate::wire::{self, Decode, Encode, Plain};

/// What settled for one ticket: a value (an async func's result, a future's value or a stream's
/// next item, encoded), or the end of a stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settled {
    pub ticket: u64,
    pub value: Option<Vec<u8>>,
}

/// The host's side of an async func's or a future's ticket: settled once.
pub struct Promise<T, S = Plain> {
    ticket: u64,
    shape: PhantomData<fn(&T) -> S>,
}

impl<T, S: Encode<T>> Promise<T, S> {
    pub fn new(ticket: u64) -> Self {
        Self {
            ticket,
            shape: PhantomData,
        }
    }

    pub fn ticket(&self) -> u64 {
        self.ticket
    }

    pub fn settle(self, value: &T) -> Settled {
        Settled {
            ticket: self.ticket,
            value: Some(wire::encode::<S, T>(value)),
        }
    }
}

/// The host's side of a stream's ticket: any number of items, then its end.
pub struct Sender<T, S = Plain> {
    ticket: u64,
    shape: PhantomData<fn(&T) -> S>,
}

impl<T, S: Encode<T>> Sender<T, S> {
    pub fn new(ticket: u64) -> Self {
        Self {
            ticket,
            shape: PhantomData,
        }
    }

    pub fn ticket(&self) -> u64 {
        self.ticket
    }

    pub fn send(&self, value: &T) -> Settled {
        Settled {
            ticket: self.ticket,
            value: Some(wire::encode::<S, T>(value)),
        }
    }

    pub fn end(self) -> Settled {
        Settled {
            ticket: self.ticket,
            value: None,
        }
    }
}

type Waiting = Box<dyn FnMut(Option<&[u8]>) -> Kept>;

#[derive(PartialEq, Eq)]
enum Kept {
    Keep,
    Done,
}

/// The guest's tickets of one local interface, waiting for its poll.
#[derive(Default)]
pub struct Settling {
    waiting: RefCell<HashMap<u64, Waiting>>,
}

impl Settling {
    /// Hands what settled to the tickets waiting for it. A ticket nobody waits for any more
    /// (its future or stream was dropped) is ignored, as is a value that does not decode.
    pub fn settle(&self, settled: Vec<Settled>) {
        for Settled { ticket, value } in settled {
            let Some(mut waiting) = self.waiting.borrow_mut().remove(&ticket) else {
                continue;
            };
            if waiting(value.as_deref()) == Kept::Keep {
                self.waiting.borrow_mut().insert(ticket, waiting);
            }
        }
    }

    /// Whether any ticket still waits.
    pub fn waiting(&self) -> bool {
        !self.waiting.borrow().is_empty()
    }

    /// Waits for the value of an async func's or a future's ticket.
    pub fn ticket<T: 'static, S: Decode<T> + 'static>(&self, ticket: u64) -> Ticket<T> {
        let shared = Rc::new(RefCell::new(Slot::<T>::default()));
        let slot = Rc::downgrade(&shared);
        self.waiting.borrow_mut().insert(
            ticket,
            Box::new(move |value| {
                if let (Some(slot), Some(value)) = (slot.upgrade(), value) {
                    let mut slot = slot.borrow_mut();
                    slot.value = wire::decode::<S, T>(value);
                    if let Some(waker) = slot.waker.take() {
                        waker.wake();
                    }
                }
                Kept::Done
            }),
        );
        Ticket { slot: shared }
    }

    /// Waits for the items of a stream's ticket.
    pub fn items<T: 'static, S: Decode<T> + 'static>(&self, ticket: u64) -> Items<T> {
        let shared = Rc::new(RefCell::new(Queue::<T>::default()));
        let queue = Rc::downgrade(&shared);
        self.waiting.borrow_mut().insert(
            ticket,
            Box::new(move |value| {
                let Some(queue) = queue.upgrade() else {
                    return Kept::Done;
                };
                let mut queue = queue.borrow_mut();
                let kept = match value {
                    Some(value) => {
                        if let Some(item) = wire::decode::<S, T>(value) {
                            queue.items.push_back(item);
                        }
                        Kept::Keep
                    }
                    None => {
                        queue.ended = true;
                        Kept::Done
                    }
                };
                if let Some(waker) = queue.waker.take() {
                    waker.wake();
                }
                kept
            }),
        );
        Items { queue: shared }
    }
}

struct Slot<T> {
    value: Option<T>,
    waker: Option<Waker>,
}

impl<T> Default for Slot<T> {
    fn default() -> Self {
        Self {
            value: None,
            waker: None,
        }
    }
}

/// An async func's result or a future's value, once the interface's poll has it.
pub struct Ticket<T> {
    slot: Rc<RefCell<Slot<T>>>,
}

impl<T> Ticket<T> {
    /// The value, if it has settled, without waiting.
    pub fn take(&mut self) -> Option<T> {
        self.slot.borrow_mut().value.take()
    }
}

impl<T> Future for Ticket<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<T> {
        let mut slot = self.slot.borrow_mut();
        match slot.value.take() {
            Some(value) => Poll::Ready(value),
            None => {
                slot.waker = Some(context.waker().clone());
                Poll::Pending
            }
        }
    }
}

struct Queue<T> {
    items: VecDeque<T>,
    ended: bool,
    waker: Option<Waker>,
}

impl<T> Default for Queue<T> {
    fn default() -> Self {
        Self {
            items: VecDeque::new(),
            ended: false,
            waker: None,
        }
    }
}

/// A stream's items, as the interface's poll brings them.
pub struct Items<T> {
    queue: Rc<RefCell<Queue<T>>>,
}

impl<T> Items<T> {
    /// The next item that has arrived, without waiting.
    pub fn try_next(&mut self) -> Option<T> {
        self.queue.borrow_mut().items.pop_front()
    }

    /// Whether the stream has ended and every item has been taken.
    pub fn ended(&self) -> bool {
        let queue = self.queue.borrow();
        queue.ended && queue.items.is_empty()
    }

    /// The next item, or none once the stream has ended.
    pub fn next(&mut self) -> Next<'_, T> {
        Next { items: self }
    }
}

pub struct Next<'a, T> {
    items: &'a mut Items<T>,
}

impl<T> Future for Next<'_, T> {
    type Output = Option<T>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<T>> {
        let mut queue = self.items.queue.borrow_mut();
        if let Some(item) = queue.items.pop_front() {
            return Poll::Ready(Some(item));
        }
        if queue.ended {
            return Poll::Ready(None);
        }
        queue.waker = Some(context.waker().clone());
        Poll::Pending
    }
}

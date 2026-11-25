//! The async half of a local interface (guides/type_modules.md, "Local interfaces"): what its
//! async funcs, futures and streams answer arrives through the interface's one generated
//! `[poll]` import, as a list of [`Settled`].
//!
//! The host answers each such call at once with a ticket, unique within the interface and never
//! [`REFUSED`], and settles it later with a [`Promise`] (an async func or a future) or a
//! [`Sender`] (a stream). The guest's call answers a [`Ticket`] or an [`Items`], which the
//! generated `settle` fills from a [`Settling`].

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

/// The ticket a call answers when the host refused it (its arguments did not lift) or could not
/// answer it (it has no gpu): nothing will ever settle it, so the guest stops there.
pub const REFUSED: u32 = 0;

/// What settled for one ticket: a value (an async func's result, a future's value or a stream's
/// next item, encoded), or the end of a stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settled {
    pub ticket: u32,
    pub value: Option<Vec<u8>>,
}

/// The host's side of an async func's or a future's ticket: settled once. The host settles every
/// promise it holds, failing them when the device they belong to is lost, since a guest waits
/// for each.
#[must_use = "a guest waits until its promise settles"]
pub struct Promise<T, S = Plain> {
    ticket: u32,
    shape: PhantomData<fn(&T) -> S>,
}

impl<T, S: Encode<T>> Promise<T, S> {
    pub fn new(ticket: u32) -> Self {
        Self {
            ticket,
            shape: PhantomData,
        }
    }

    pub fn ticket(&self) -> u32 {
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
#[must_use = "a guest waits for a stream's items until it ends"]
pub struct Sender<T, S = Plain> {
    ticket: u32,
    shape: PhantomData<fn(&T) -> S>,
}

impl<T, S: Encode<T>> Sender<T, S> {
    pub fn new(ticket: u32) -> Self {
        Self {
            ticket,
            shape: PhantomData,
        }
    }

    pub fn ticket(&self) -> u32 {
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

type Table = Rc<RefCell<HashMap<u32, Waiting>>>;

/// The guest's tickets of one local interface, waiting for its poll.
#[derive(Default)]
pub struct Settling {
    waiting: Table,
}

impl Settling {
    /// Hands what settled to the tickets waiting for it. A ticket nobody waits for any more (its
    /// future or stream was dropped) is ignored.
    ///
    /// # Panics
    ///
    /// If a value does not decode as its ticket's type: the host and guest disagree on the
    /// interface.
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

    /// Whether any future or stream still waits for a ticket.
    pub fn waiting(&self) -> bool {
        !self.waiting.borrow().is_empty()
    }

    /// Waits for the value of an async func's or a future's ticket.
    ///
    /// # Panics
    ///
    /// If the ticket is [`REFUSED`], naming `call`.
    pub fn ticket<T: 'static, S: Decode<T> + 'static>(&self, call: &str, ticket: u32) -> Ticket<T> {
        refused(call, ticket);
        let shared = Rc::new(RefCell::new(Slot::<T>::default()));
        let slot = Rc::downgrade(&shared);
        let name = call.to_owned();
        self.waiting.borrow_mut().insert(
            ticket,
            Box::new(move |value| {
                let (Some(slot), Some(value)) = (slot.upgrade(), value) else {
                    return Kept::Done;
                };
                let value = wire::decode::<S, T>(value)
                    .unwrap_or_else(|| panic!("{name} settled with what does not decode"));
                let waker = {
                    let mut slot = slot.borrow_mut();
                    slot.value = Some(value);
                    slot.waker.take()
                };
                if let Some(waker) = waker {
                    waker.wake();
                }
                Kept::Done
            }),
        );
        Ticket {
            slot: shared,
            entry: Entry {
                table: Rc::clone(&self.waiting),
                ticket,
            },
        }
    }

    /// Waits for the items of a stream's ticket.
    ///
    /// # Panics
    ///
    /// If the ticket is [`REFUSED`], naming `call`.
    pub fn items<T: 'static, S: Decode<T> + 'static>(&self, call: &str, ticket: u32) -> Items<T> {
        refused(call, ticket);
        let shared = Rc::new(RefCell::new(Queue::<T>::default()));
        let queue = Rc::downgrade(&shared);
        let name = call.to_owned();
        self.waiting.borrow_mut().insert(
            ticket,
            Box::new(move |value| {
                let Some(queue) = queue.upgrade() else {
                    return Kept::Done;
                };
                let kept = match value {
                    Some(value) => {
                        let item = wire::decode::<S, T>(value)
                            .unwrap_or_else(|| panic!("{name} sent what does not decode"));
                        queue.borrow_mut().items.push_back(item);
                        Kept::Keep
                    }
                    None => {
                        queue.borrow_mut().ended = true;
                        Kept::Done
                    }
                };
                let waker = queue.borrow_mut().waker.take();
                if let Some(waker) = waker {
                    waker.wake();
                }
                kept
            }),
        );
        Items {
            queue: shared,
            entry: Entry {
                table: Rc::clone(&self.waiting),
                ticket,
            },
        }
    }
}

fn refused(call: &str, ticket: u32) {
    if ticket == REFUSED {
        panic!("{call} was refused by the host, or the host had nothing to answer it with");
    }
}

/// A ticket's place in its interface's table, which a dropped future or stream gives up, so
/// `Settling::waiting` stops counting it.
struct Entry {
    table: Table,
    ticket: u32,
}

impl Drop for Entry {
    fn drop(&mut self) {
        if let Ok(mut table) = self.table.try_borrow_mut() {
            table.remove(&self.ticket);
        }
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
    entry: Entry,
}

impl<T> Ticket<T> {
    /// The value, if it has settled, without waiting.
    pub fn take(&mut self) -> Option<T> {
        self.slot.borrow_mut().value.take()
    }

    /// The ticket the host answered the call with.
    pub fn ticket(&self) -> u32 {
        self.entry.ticket
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
    entry: Entry,
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

    /// The ticket the host answered the call with.
    pub fn ticket(&self) -> u32 {
        self.entry.ticket
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

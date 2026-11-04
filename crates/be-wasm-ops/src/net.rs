use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
    future::Future,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    pin::Pin,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    task::{Context, Poll, Waker},
};

use serde::Serialize;

use crate::{
    Connection, FrameError, Incoming, MethodId, OpError, OpId, Outcome, STREAM_WINDOW,
    STREAM_WINDOW_BYTES, Side, encode,
    wire::{Decode, Plain},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Arrival {
    Item(Vec<u8>),
    Done(Outcome),
}

pub trait Caller: Send + Sync + 'static {
    fn start(&self, parent: Option<OpId>, method: MethodId, args: Vec<u8>) -> OpId;

    fn poll_op(&self, op: OpId, context: &mut Context<'_>) -> Poll<Arrival>;

    fn credit(&self, op: OpId, items: u32, bytes: u64);

    fn cancel(&self, op: OpId);

    fn forget(&self, op: OpId);

    fn request_cancel(&self, op: OpId);
}

impl<C: Caller + ?Sized> Caller for Arc<C> {
    fn start(&self, parent: Option<OpId>, method: MethodId, args: Vec<u8>) -> OpId {
        (**self).start(parent, method, args)
    }

    fn poll_op(&self, op: OpId, context: &mut Context<'_>) -> Poll<Arrival> {
        (**self).poll_op(op, context)
    }

    fn credit(&self, op: OpId, items: u32, bytes: u64) {
        (**self).credit(op, items, bytes);
    }

    fn cancel(&self, op: OpId) {
        (**self).cancel(op);
    }

    fn forget(&self, op: OpId) {
        (**self).forget(op);
    }

    fn request_cancel(&self, op: OpId) {
        (**self).request_cancel(op);
    }
}

pub fn share(caller: &(impl Caller + Clone)) -> Arc<dyn Caller> {
    Arc::new(caller.clone())
}

pub fn call<T, S: Decode<T>, A: Serialize>(
    caller: Arc<dyn Caller>,
    parent: Option<OpId>,
    method: MethodId,
    args: &A,
) -> Pending<T, S> {
    let op = caller.start(parent, method, encode(args));
    Pending {
        op,
        caller,
        finished: false,
        value: PhantomData,
    }
}

pub fn stream<T, S: Decode<T>, A: Serialize>(
    caller: Arc<dyn Caller>,
    parent: Option<OpId>,
    method: MethodId,
    args: &A,
) -> Stream<T, S> {
    let op = caller.start(parent, method, encode(args));
    Stream {
        op,
        caller,
        ended: None,
        taken: 0,
        taken_bytes: 0,
        item: PhantomData,
    }
}

pub fn open<A: Serialize>(caller: Arc<dyn Caller>, method: MethodId, args: &A) -> Resource {
    let op = caller.start(None, method, encode(args));
    Resource {
        op,
        caller,
        ended: None,
    }
}

#[must_use = "dropping a call asks the other side to cancel it; `.detach()` says nobody waits for its answer"]
pub struct Pending<T, S = Plain> {
    op: OpId,
    caller: Arc<dyn Caller>,
    finished: bool,
    value: PhantomData<fn() -> (T, S)>,
}

impl<T, S> fmt::Debug for Pending<T, S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Pending({})", self.op)
    }
}

impl<T, S> Pending<T, S> {
    pub fn op(&self) -> OpId {
        self.op
    }

    pub fn detach(mut self) {
        self.finished = true;
        self.caller.forget(self.op);
    }
}

impl<T, S> Unpin for Pending<T, S> {}

impl<T, S: Decode<T>> Future for Pending<T, S> {
    type Output = Result<T, OpError>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        if self.finished {
            return Poll::Ready(Err(OpError::Canceled));
        }
        loop {
            match self.caller.poll_op(self.op, context) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Arrival::Item(_)) => continue,
                Poll::Ready(Arrival::Done(outcome)) => {
                    self.finished = true;
                    return Poll::Ready(outcome.decode_as::<S, T>());
                }
            }
        }
    }
}

impl<T, S> Drop for Pending<T, S> {
    fn drop(&mut self) {
        if !self.finished {
            self.caller.cancel(self.op);
        }
    }
}

pub struct Stream<T, S = Plain> {
    op: OpId,
    caller: Arc<dyn Caller>,
    ended: Option<Result<(), OpError>>,
    taken: u32,
    taken_bytes: u64,
    item: PhantomData<fn() -> (T, S)>,
}

impl<T, S> fmt::Debug for Stream<T, S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Stream({})", self.op)
    }
}

impl<T, S> Unpin for Stream<T, S> {}

impl<T, S: Decode<T>> Stream<T, S> {
    pub fn op(&self) -> OpId {
        self.op
    }

    pub fn ended(&self) -> Option<Result<(), OpError>> {
        self.ended
    }

    pub fn recv(&mut self) -> Next<'_, T, S> {
        Next { stream: self }
    }

    pub fn poll_recv(&mut self, context: &mut Context<'_>) -> Poll<Option<T>> {
        if self.ended.is_some() {
            return Poll::Ready(None);
        }
        match self.caller.poll_op(self.op, context) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Arrival::Done(outcome)) => {
                self.ended = Some(outcome.decode::<()>());
                Poll::Ready(None)
            }
            Poll::Ready(Arrival::Item(item)) => {
                self.taken += 1;
                self.taken_bytes += item.len() as u64;
                if self.taken >= STREAM_WINDOW / 2 || self.taken_bytes >= STREAM_WINDOW_BYTES / 2 {
                    self.caller.credit(
                        self.op,
                        std::mem::take(&mut self.taken),
                        std::mem::take(&mut self.taken_bytes),
                    );
                }
                match crate::wire::decode::<S, T>(&item) {
                    Some(item) => Poll::Ready(Some(item)),
                    None => {
                        self.caller.cancel(self.op);
                        self.ended = Some(Err(OpError::Unsupported));
                        Poll::Ready(None)
                    }
                }
            }
        }
    }
}

impl<T, S: Decode<T>> futures_core::Stream for Stream<T, S> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<T>> {
        self.poll_recv(context)
    }
}

impl<T, S> Drop for Stream<T, S> {
    fn drop(&mut self) {
        if self.ended.is_none() {
            self.caller.cancel(self.op);
        }
    }
}

pub struct Next<'a, T, S = Plain> {
    stream: &'a mut Stream<T, S>,
}

impl<T, S: Decode<T>> Future for Next<'_, T, S> {
    type Output = Option<T>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        self.stream.poll_recv(context)
    }
}

pub struct Resource {
    op: OpId,
    caller: Arc<dyn Caller>,
    ended: Option<Result<(), OpError>>,
}

impl fmt::Debug for Resource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Resource({})", self.op)
    }
}

impl Resource {
    pub fn op(&self) -> OpId {
        self.op
    }

    pub fn caller(&self) -> Arc<dyn Caller> {
        Arc::clone(&self.caller)
    }

    pub fn closed(&mut self) -> Closed<'_> {
        Closed { resource: self }
    }

    pub fn close(mut self) -> impl Future<Output = Result<(), OpError>> + Send {
        async move {
            if self.ended.is_none() {
                self.caller.request_cancel(self.op);
            }
            self.closed().await
        }
    }
}

impl Drop for Resource {
    fn drop(&mut self) {
        if self.ended.is_none() {
            self.caller.cancel(self.op);
        }
    }
}

pub struct Closed<'a> {
    resource: &'a mut Resource,
}

impl Future for Closed<'_> {
    type Output = Result<(), OpError>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let resource = &mut *self.resource;
        loop {
            if let Some(ended) = resource.ended {
                return Poll::Ready(ended);
            }
            match resource.caller.poll_op(resource.op, context) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Arrival::Item(_)) => continue,
                Poll::Ready(Arrival::Done(outcome)) => {
                    resource.ended = Some(outcome.decode::<()>());
                }
            }
        }
    }
}

type Sink = Box<dyn Fn(Vec<u8>) + Send + Sync>;

#[derive(Default)]
struct Inbox {
    items: VecDeque<Vec<u8>>,
    outcome: Option<Outcome>,
    waker: Option<Waker>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Want {
    Room,
    Cancel,
}

struct State {
    connection: Connection,
    calls: BTreeMap<OpId, Inbox>,
    forgotten: BTreeSet<OpId>,
    waiting: BTreeMap<OpId, Vec<(Want, Waker)>>,
    closed: bool,
}

impl State {
    fn settled(&self, op: OpId, want: Want) -> bool {
        let connection = &self.connection;
        self.closed
            || !connection.is_serving(op)
            || connection.is_canceled(op)
            || (want == Want::Room && connection.ready(op))
    }

    fn flush(&mut self, sink: &Sink) {
        let outbound = self.connection.take_outbound();
        if !self.closed {
            for frame in outbound {
                sink(frame);
            }
        }
        let ops: Vec<OpId> = self.waiting.keys().copied().collect();
        for op in ops {
            let waiters = self.waiting.remove(&op).unwrap_or_default();
            let (woken, kept): (Vec<_>, Vec<_>) = waiters
                .into_iter()
                .partition(|(want, _)| self.settled(op, *want));
            for (_, waker) in woken {
                waker.wake();
            }
            if !kept.is_empty() {
                self.waiting.insert(op, kept);
            }
        }
    }
}

struct Shared {
    state: Mutex<State>,
    sink: Sink,
}

#[derive(Clone)]
pub struct Peer(Arc<Shared>);

impl fmt::Debug for Peer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Peer")
    }
}

impl Peer {
    pub fn new(side: Side, sink: impl Fn(Vec<u8>) + Send + Sync + 'static) -> Self {
        Self(Arc::new(Shared {
            state: Mutex::new(State {
                connection: Connection::new(side),
                calls: BTreeMap::new(),
                forgotten: BTreeSet::new(),
                waiting: BTreeMap::new(),
                closed: false,
            }),
            sink: Box::new(sink),
        }))
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.0.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn lock(&self) -> Locked<'_> {
        Locked {
            state: self.state(),
            sink: &self.0.sink,
        }
    }

    pub fn receive(&self, frame: &[u8]) -> Result<Vec<Incoming>, FrameError> {
        let mut state = self.state();
        let received = state.connection.receive(frame);
        let mut events = Vec::new();
        while let Some(event) = state.connection.next_event() {
            let op = match &event {
                Incoming::Yielded { op, .. } | Incoming::Completed { op, .. } => *op,
                _ => {
                    events.push(event);
                    continue;
                }
            };
            let State {
                calls, forgotten, ..
            } = &mut *state;
            match (calls.get_mut(&op), event) {
                (Some(inbox), Incoming::Yielded { item, .. }) => {
                    inbox.items.push_back(item);
                    if let Some(waker) = inbox.waker.take() {
                        waker.wake();
                    }
                }
                (Some(inbox), Incoming::Completed { outcome, .. }) => {
                    inbox.outcome = Some(outcome);
                    if let Some(waker) = inbox.waker.take() {
                        waker.wake();
                    }
                }
                (None, Incoming::Completed { .. }) if forgotten.remove(&op) => {}
                (None, Incoming::Yielded { .. }) if forgotten.contains(&op) => {}
                (None, event) => events.push(event),
                (Some(_), event) => events.push(event),
            }
        }
        state.flush(&self.0.sink);
        received.map(|()| events)
    }

    pub fn close(&self) {
        let mut state = self.state();
        state.closed = true;
        for inbox in state.calls.values_mut() {
            inbox.outcome.get_or_insert(Outcome::Err(OpError::Canceled));
            if let Some(waker) = inbox.waker.take() {
                waker.wake();
            }
        }
        state.flush(&self.0.sink);
    }

    pub fn writable(&self, op: OpId) -> Writable<'_> {
        Writable { peer: self, op }
    }

    pub fn canceled(&self, op: OpId) -> Canceled<'_> {
        Canceled { peer: self, op }
    }

    fn wait(&self, op: OpId, want: Want, context: &mut Context<'_>) -> Poll<bool> {
        let mut state = self.state();
        if state.settled(op, want) {
            let room = want == Want::Room
                && !state.closed
                && state.connection.is_serving(op)
                && !state.connection.is_canceled(op);
            return Poll::Ready(room);
        }
        let waiters = state.waiting.entry(op).or_default();
        if !waiters
            .iter()
            .any(|(wanted, waker)| *wanted == want && waker.will_wake(context.waker()))
        {
            waiters.push((want, context.waker().clone()));
        }
        Poll::Pending
    }
}

impl Caller for Peer {
    fn start(&self, parent: Option<OpId>, method: MethodId, args: Vec<u8>) -> OpId {
        let mut state = self.state();
        let op = state.connection.start(parent, method, args);
        let outcome = state.closed.then_some(Outcome::Err(OpError::Canceled));
        state.calls.insert(
            op,
            Inbox {
                outcome,
                ..Inbox::default()
            },
        );
        state.flush(&self.0.sink);
        op
    }

    fn poll_op(&self, op: OpId, context: &mut Context<'_>) -> Poll<Arrival> {
        let mut state = self.state();
        let Some(inbox) = state.calls.get_mut(&op) else {
            return Poll::Ready(Arrival::Done(Outcome::Err(OpError::Canceled)));
        };
        if let Some(item) = inbox.items.pop_front() {
            return Poll::Ready(Arrival::Item(item));
        }
        if let Some(outcome) = inbox.outcome.take() {
            state.calls.remove(&op);
            return Poll::Ready(Arrival::Done(outcome));
        }
        inbox.waker = Some(context.waker().clone());
        Poll::Pending
    }

    fn credit(&self, op: OpId, items: u32, bytes: u64) {
        let mut state = self.state();
        state.connection.credit(op, items, bytes);
        state.flush(&self.0.sink);
    }

    fn cancel(&self, op: OpId) {
        let mut state = self.state();
        if state.calls.remove(&op).is_none() {
            return;
        }
        if state.connection.is_pending(op) {
            state.forgotten.insert(op);
            state.connection.cancel(op);
        }
        state.flush(&self.0.sink);
    }

    fn forget(&self, op: OpId) {
        let mut state = self.state();
        if state.calls.remove(&op).is_some() && state.connection.is_pending(op) {
            state.forgotten.insert(op);
        }
    }

    fn request_cancel(&self, op: OpId) {
        let mut state = self.state();
        if state.calls.contains_key(&op) {
            state.connection.cancel(op);
        }
        state.flush(&self.0.sink);
    }
}

pub struct Locked<'a> {
    state: MutexGuard<'a, State>,
    sink: &'a Sink,
}

impl Deref for Locked<'_> {
    type Target = Connection;

    fn deref(&self) -> &Connection {
        &self.state.connection
    }
}

impl DerefMut for Locked<'_> {
    fn deref_mut(&mut self) -> &mut Connection {
        &mut self.state.connection
    }
}

impl Drop for Locked<'_> {
    fn drop(&mut self) {
        self.state.flush(self.sink);
    }
}

pub struct Writable<'a> {
    peer: &'a Peer,
    op: OpId,
}

impl Future for Writable<'_> {
    type Output = Result<(), OpError>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        self.peer
            .wait(self.op, Want::Room, context)
            .map(|room| match room {
                true => Ok(()),
                false => Err(OpError::Canceled),
            })
    }
}

pub struct Canceled<'a> {
    peer: &'a Peer,
    op: OpId,
}

impl Future for Canceled<'_> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        self.peer.wait(self.op, Want::Cancel, context).map(|_| ())
    }
}

use std::{
    any::Any,
    cell::{Cell, RefCell},
    collections::{BTreeMap, VecDeque},
    future::Future,
    marker::PhantomData,
    pin::Pin,
    rc::Rc,
    sync::{Arc, Mutex},
    task::{Context, Poll, Wake, Waker},
};

use serde::{Serialize, de::DeserializeOwned};

use crate::{
    Called, Connection, Incoming, MethodId, OpError, OpId, Outcome, Side, Started, encode,
};

pub type LocalFuture<T> = Pin<Box<dyn Future<Output = T>>>;

pub enum Serve {
    Done(Outcome),
    Later(LocalFuture<Outcome>),
    Resource {
        instance: Rc<dyn Any>,
        on_drop: Box<dyn FnOnce() -> LocalFuture<()>>,
    },
}

pub type Dispatch = Box<dyn Fn(&Resources, Started) -> Serve>;

pub struct Resources<'a>(&'a RefCell<BTreeMap<OpId, Rc<dyn Any>>>);

impl Resources<'_> {
    pub fn get<T: 'static>(&self, op: Option<OpId>) -> Option<Rc<T>> {
        let instance = Rc::clone(self.0.borrow().get(&op?)?);
        instance.downcast().ok()
    }
}

#[derive(Default)]
struct Call {
    outcome: Option<Outcome>,
    items: VecDeque<Vec<u8>>,
    taken: u32,
    taken_bytes: u64,
    waker: Option<Waker>,
}

enum Serving {
    Task(u64),
    Resource {
        on_drop: Option<Box<dyn FnOnce() -> LocalFuture<()>>>,
    },
}

struct ReadyQueue(Mutex<VecDeque<u64>>);

struct TaskWaker {
    task: u64,
    ready: Arc<ReadyQueue>,
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.ready
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push_back(self.task);
    }
}

struct Inner {
    connection: RefCell<Connection>,
    calls: RefCell<BTreeMap<OpId, Call>>,
    serving: RefCell<BTreeMap<OpId, Serving>>,
    resources: RefCell<BTreeMap<OpId, Rc<dyn Any>>>,
    tasks: RefCell<BTreeMap<u64, Option<LocalFuture<()>>>>,
    next_task: Cell<u64>,
    ready: Arc<ReadyQueue>,
    parent: Cell<Option<OpId>>,
    returning: RefCell<Vec<Outcome>>,
    dispatch: RefCell<Option<Dispatch>>,
}

thread_local! {
    static CURRENT: RefCell<Vec<Rc<Inner>>> = const { RefCell::new(Vec::new()) };
}

fn current() -> Rc<Inner> {
    CURRENT.with(|current| {
        current
            .borrow()
            .last()
            .cloned()
            .expect("an import is called from inside an executor's step")
    })
}

struct Entered;

impl Entered {
    fn enter(inner: &Rc<Inner>) -> Self {
        CURRENT.with(|current| current.borrow_mut().push(Rc::clone(inner)));
        Entered
    }
}

impl Drop for Entered {
    fn drop(&mut self) {
        CURRENT.with(|current| current.borrow_mut().pop());
    }
}

pub struct Executor {
    inner: Rc<Inner>,
}

impl Executor {
    pub fn new(dispatch: Dispatch) -> Self {
        let executor = Self {
            inner: Rc::new(Inner {
                connection: RefCell::new(Connection::new(Side::Guest)),
                calls: RefCell::new(BTreeMap::new()),
                serving: RefCell::new(BTreeMap::new()),
                resources: RefCell::new(BTreeMap::new()),
                tasks: RefCell::new(BTreeMap::new()),
                next_task: Cell::new(0),
                ready: Arc::new(ReadyQueue(Mutex::new(VecDeque::new()))),
                parent: Cell::new(None),
                returning: RefCell::new(Vec::new()),
                dispatch: RefCell::new(Some(dispatch)),
            }),
        };
        SERVED.with(|served| *served.borrow_mut() = Rc::downgrade(&executor.inner));
        executor
    }

    pub fn step(&self, frames: Vec<Vec<u8>>, outbound: &mut Vec<Vec<u8>>) -> Result<(), String> {
        let _entered = Entered::enter(&self.inner);
        let mut failure = None;
        for frame in frames {
            if let Err(error) = self.inner.connection.borrow_mut().receive(&frame) {
                failure.get_or_insert(error.to_string());
                continue;
            }
            self.inner.settle();
            for outcome in std::mem::take(&mut *self.inner.returning.borrow_mut()) {
                self.inner.connection.borrow_mut().return_direct(&outcome);
            }
        }
        self.inner.settle();
        outbound.extend(self.inner.connection.borrow_mut().take_outbound());
        match failure {
            Some(failure) => Err(failure),
            None => Ok(()),
        }
    }

    pub fn enter<R>(&self, run: impl FnOnce() -> R) -> R {
        let _entered = Entered::enter(&self.inner);
        let result = run();
        self.inner.settle();
        result
    }
}

impl Drop for Executor {
    fn drop(&mut self) {
        let dispatch = self.inner.dispatch.borrow_mut().take();
        let tasks = std::mem::take(&mut *self.inner.tasks.borrow_mut());
        let resources = std::mem::take(&mut *self.inner.resources.borrow_mut());
        let serving = std::mem::take(&mut *self.inner.serving.borrow_mut());
        drop((dispatch, tasks, resources, serving));
    }
}

impl Inner {
    fn settle(self: &Rc<Self>) {
        loop {
            let event = self.connection.borrow_mut().next_event();
            if let Some(event) = event {
                self.handle(event);
                continue;
            }
            let task = self
                .ready
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .pop_front();
            match task {
                Some(task) => self.poll(task),
                None => return,
            }
        }
    }

    fn call_direct(self: &Rc<Self>, called: Called) -> Outcome {
        let started = Started {
            op: 0,
            parent: called.parent,
            method: called.method(),
            args: called.args,
        };
        let served = match self.dispatch.borrow().as_ref() {
            Some(dispatch) => dispatch(&Resources(&self.resources), started),
            None => Serve::Done(Outcome::Err(OpError::Canceled)),
        };
        match served {
            Serve::Done(outcome) => outcome,
            Serve::Later(_) | Serve::Resource { .. } => Outcome::Err(OpError::Unsupported),
        }
    }

    fn handle(self: &Rc<Self>, event: Incoming) {
        match event {
            Incoming::Called(called) => {
                let answered = called.answered;
                let outcome = self.call_direct(called);
                if answered {
                    self.returning.borrow_mut().push(outcome);
                }
            }
            Incoming::Started(started) => {
                let op = started.op;
                let served = match self.dispatch.borrow().as_ref() {
                    Some(dispatch) => dispatch(&Resources(&self.resources), started),
                    None => Serve::Done(Outcome::Err(OpError::Canceled)),
                };
                match served {
                    Serve::Done(outcome) => self.connection.borrow_mut().complete(op, outcome),
                    Serve::Later(future) => {
                        let inner = Rc::downgrade(self);
                        let task = self.spawn(Box::pin(async move {
                            let outcome = future.await;
                            if let Some(inner) = inner.upgrade() {
                                inner.serving.borrow_mut().remove(&op);
                                inner.connection.borrow_mut().complete(op, outcome);
                            }
                        }));
                        self.serving.borrow_mut().insert(op, Serving::Task(task));
                    }
                    Serve::Resource { instance, on_drop } => {
                        self.resources.borrow_mut().insert(op, instance);
                        self.serving.borrow_mut().insert(
                            op,
                            Serving::Resource {
                                on_drop: Some(on_drop),
                            },
                        );
                    }
                }
            }
            Incoming::Yielded { op, item } => {
                let waker = self.calls.borrow_mut().get_mut(&op).and_then(|call| {
                    call.items.push_back(item);
                    call.waker.take()
                });
                if let Some(waker) = waker {
                    waker.wake();
                }
            }
            Incoming::Completed { op, outcome } => {
                let waker = self.calls.borrow_mut().get_mut(&op).and_then(|call| {
                    call.outcome = Some(outcome);
                    call.waker.take()
                });
                if let Some(waker) = waker {
                    waker.wake();
                }
            }
            Incoming::Credited { .. } => {}
            Incoming::Canceled { op } => {
                let serving = self.serving.borrow_mut().remove(&op);
                match serving {
                    Some(Serving::Task(task)) => {
                        let future = self.tasks.borrow_mut().remove(&task);
                        drop(future);
                        self.connection
                            .borrow_mut()
                            .complete(op, Outcome::Err(OpError::Canceled));
                    }
                    Some(Serving::Resource { mut on_drop }) => {
                        let closing = on_drop.take();
                        self.serving
                            .borrow_mut()
                            .insert(op, Serving::Resource { on_drop: None });
                        if let Some(closing) = closing {
                            let inner = Rc::downgrade(self);
                            let closed = closing();
                            self.spawn(Box::pin(async move {
                                closed.await;
                                if let Some(inner) = inner.upgrade() {
                                    inner.end(op);
                                }
                            }));
                        }
                    }
                    None => {}
                }
            }
        }
    }

    fn end(&self, op: OpId) {
        let resource = self.resources.borrow_mut().remove(&op);
        self.serving.borrow_mut().remove(&op);
        self.connection
            .borrow_mut()
            .complete(op, Outcome::returned(&()));
        drop(resource);
    }

    fn spawn(&self, future: LocalFuture<()>) -> u64 {
        let task = self.next_task.get() + 1;
        self.next_task.set(task);
        self.tasks.borrow_mut().insert(task, Some(future));
        self.ready
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push_back(task);
        task
    }

    fn poll(&self, task: u64) {
        let Some(mut future) = self
            .tasks
            .borrow_mut()
            .get_mut(&task)
            .and_then(Option::take)
        else {
            return;
        };
        let waker = Waker::from(Arc::new(TaskWaker {
            task,
            ready: Arc::clone(&self.ready),
        }));
        let mut context = Context::from_waker(&waker);
        let parent = self.parent.replace(None);
        let polled = future.as_mut().poll(&mut context);
        self.parent.set(parent);
        match polled {
            Poll::Ready(()) => {
                self.tasks.borrow_mut().remove(&task);
            }
            Poll::Pending => {
                let mut tasks = self.tasks.borrow_mut();
                if let Some(slot) = tasks.get_mut(&task) {
                    *slot = Some(future);
                }
            }
        }
    }

    fn start(&self, method: MethodId, args: Vec<u8>) -> OpId {
        let op = self
            .connection
            .borrow_mut()
            .start(self.parent.get(), method, args);
        self.calls.borrow_mut().insert(op, Call::default());
        op
    }

    fn forget(&self, op: OpId) {
        if self.calls.borrow_mut().remove(&op).is_some() {
            self.connection.borrow_mut().cancel(op);
        }
    }
}

pub fn spawn(future: impl Future<Output = ()> + 'static) {
    current().spawn(Box::pin(future));
}

pub fn with_parent<R>(parent: OpId, run: impl FnOnce() -> R) -> R {
    let inner = current();
    let before = inner.parent.replace(Some(parent));
    let result = run();
    inner.parent.set(before);
    result
}

pub fn end(resource: OpId) {
    let inner = current();
    let open = matches!(
        inner.serving.borrow().get(&resource),
        Some(Serving::Resource { .. })
    );
    if open {
        inner.end(resource);
    }
}

#[derive(Clone, Copy)]
pub struct Imports {
    pub call: fn(&str, i32, i64, u32, u32),
    pub send: fn(&[u8]),
}

thread_local! {
    static IMPORTS: Cell<Option<Imports>> = const { Cell::new(None) };
    static SERVED: RefCell<std::rc::Weak<Inner>> = const { RefCell::new(std::rc::Weak::new()) };
    static RESULT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

pub fn register_imports(imports: Imports) {
    IMPORTS.with(|held| held.set(Some(imports)));
}

fn flush(inner: &Inner) {
    if let Some(imports) = IMPORTS.with(Cell::get) {
        for frame in inner.connection.borrow_mut().take_outbound() {
            (imports.send)(&frame);
        }
    }
}

pub fn direct(name: &str, args: &[u8]) {
    let inner = current();
    let parent = inner.parent.get();
    match IMPORTS.with(Cell::get) {
        Some(imports) => {
            flush(&inner);
            (imports.call)(
                name,
                parent.is_some() as i32,
                parent.unwrap_or_default() as i64,
                args.as_ptr() as u32,
                args.len() as u32,
            );
        }
        None => inner
            .connection
            .borrow_mut()
            .notify_direct(parent, name, args),
    }
}

pub fn alloc(length: u32) -> u32 {
    let buffer = vec![0u8; length as usize].into_boxed_slice();
    Box::into_raw(buffer) as *mut u8 as usize as u32
}

// unsafe(ffi)
pub fn export(name: &str, has_parent: i32, parent: i64, pointer: u32, length: u32) -> i64 {
    let args = unsafe {
        Box::from_raw(std::ptr::slice_from_raw_parts_mut(
            pointer as usize as *mut u8,
            length as usize,
        ))
    };
    let Some(inner) = SERVED.with(|served| served.borrow().upgrade()) else {
        return block_guest_abi::direct::pack(0, 0);
    };
    let _entered = Entered::enter(&inner);
    let called = Called {
        name: name.to_owned(),
        parent: (has_parent != 0).then_some(parent as u64),
        answered: true,
        args: args.into_vec(),
    };
    let outcome = inner.call_direct(called);
    inner.settle();
    flush(&inner);
    RESULT.with(|result| {
        let mut result = result.borrow_mut();
        *result = encode(&outcome);
        block_guest_abi::direct::pack(result.as_ptr() as usize as u32, result.len() as u32)
    })
}

pub fn call<T: DeserializeOwned, A: Serialize>(method: MethodId, args: &A) -> Pending<T> {
    let inner = current();
    let op = inner.start(method, encode(args));
    Pending {
        op,
        inner,
        finished: false,
        value: PhantomData,
    }
}

pub fn stream<T: DeserializeOwned, A: Serialize>(method: MethodId, args: &A) -> Stream<T> {
    let inner = current();
    let op = inner.start(method, encode(args));
    Stream {
        op,
        inner,
        finished: false,
        item: PhantomData,
    }
}

pub fn open<A: Serialize>(method: MethodId, args: &A) -> Resource {
    let inner = current();
    let op = inner.start(method, encode(args));
    Resource { op, inner }
}

pub struct Resource {
    op: OpId,
    inner: Rc<Inner>,
}

impl Resource {
    pub fn op(&self) -> OpId {
        self.op
    }

    pub fn ended(&self) -> bool {
        self.inner
            .calls
            .borrow()
            .get(&self.op)
            .is_none_or(|call| call.outcome.is_some())
    }
}

impl Drop for Resource {
    fn drop(&mut self) {
        self.inner.forget(self.op);
    }
}

pub struct Pending<T> {
    op: OpId,
    inner: Rc<Inner>,
    finished: bool,
    value: PhantomData<fn() -> T>,
}

impl<T> Pending<T> {
    pub fn op(&self) -> OpId {
        self.op
    }
}

impl<T> Unpin for Pending<T> {}

impl<T: DeserializeOwned> Future for Pending<T> {
    type Output = Result<T, OpError>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        if self.finished {
            return Poll::Ready(Err(OpError::Canceled));
        }
        let mut calls = self.inner.calls.borrow_mut();
        let Some(call) = calls.get_mut(&self.op) else {
            drop(calls);
            self.finished = true;
            return Poll::Ready(Err(OpError::Canceled));
        };
        match call.outcome.take() {
            Some(outcome) => {
                calls.remove(&self.op);
                drop(calls);
                self.finished = true;
                Poll::Ready(outcome.decode())
            }
            None => {
                call.waker = Some(context.waker().clone());
                Poll::Pending
            }
        }
    }
}

impl<T> Drop for Pending<T> {
    fn drop(&mut self) {
        if !self.finished {
            self.inner.forget(self.op);
        }
    }
}

pub struct Stream<T> {
    op: OpId,
    inner: Rc<Inner>,
    finished: bool,
    item: PhantomData<fn() -> T>,
}

impl<T: DeserializeOwned> Stream<T> {
    pub fn op(&self) -> OpId {
        self.op
    }

    pub fn recv(&mut self) -> Next<'_, T> {
        Next { stream: self }
    }

    fn poll_next(&mut self, context: &mut Context<'_>) -> Poll<Option<T>> {
        if self.finished {
            return Poll::Ready(None);
        }
        let mut calls = self.inner.calls.borrow_mut();
        let Some(call) = calls.get_mut(&self.op) else {
            drop(calls);
            self.finished = true;
            return Poll::Ready(None);
        };
        if let Some(item) = call.items.pop_front() {
            call.taken += 1;
            call.taken_bytes += item.len() as u64;
            let granted = match call.taken >= crate::STREAM_WINDOW / 2
                || call.taken_bytes >= crate::STREAM_WINDOW_BYTES / 2
            {
                true => (
                    std::mem::take(&mut call.taken),
                    std::mem::take(&mut call.taken_bytes),
                ),
                false => (0, 0),
            };
            drop(calls);
            self.inner
                .connection
                .borrow_mut()
                .credit(self.op, granted.0, granted.1);
            return Poll::Ready(crate::decode(&item));
        }
        if call.outcome.is_some() {
            calls.remove(&self.op);
            drop(calls);
            self.finished = true;
            return Poll::Ready(None);
        }
        call.waker = Some(context.waker().clone());
        Poll::Pending
    }
}

impl<T> Drop for Stream<T> {
    fn drop(&mut self) {
        if !self.finished {
            self.inner.forget(self.op);
        }
    }
}

pub struct Next<'a, T> {
    stream: &'a mut Stream<T>,
}

impl<T: DeserializeOwned> Future for Next<'_, T> {
    type Output = Option<T>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        self.stream.poll_next(context)
    }
}

pub struct Oneshot<T> {
    slot: Rc<RefCell<(Option<T>, Option<Waker>)>>,
}

pub struct Sender<T> {
    slot: Rc<RefCell<(Option<T>, Option<Waker>)>>,
}

pub fn oneshot<T>() -> (Sender<T>, Oneshot<T>) {
    let slot = Rc::new(RefCell::new((None, None)));
    (
        Sender {
            slot: Rc::clone(&slot),
        },
        Oneshot { slot },
    )
}

impl<T> Sender<T> {
    pub fn send(self, value: T) {
        let waker = {
            let mut slot = self.slot.borrow_mut();
            slot.0 = Some(value);
            slot.1.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let waker = self.slot.borrow_mut().1.take();
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

impl<T> Future for Oneshot<T> {
    type Output = Option<T>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let mut slot = self.slot.borrow_mut();
        if let Some(value) = slot.0.take() {
            return Poll::Ready(Some(value));
        }
        if Rc::strong_count(&self.slot) == 1 {
            return Poll::Ready(None);
        }
        slot.1 = Some(context.waker().clone());
        Poll::Pending
    }
}

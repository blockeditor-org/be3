use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::rc::{Rc, Weak};

use crate::arena::{self, Key};

use crate::computation::{Computation, State, no_computation};
use crate::loading::Pending;
use crate::runtime::{RUNTIME, check_tracked_read, untrack};

pub(crate) struct Node {
    pub(crate) version: Cell<u64>,
    pub(crate) tracked_by: Cell<u64>,
    pub(crate) subscribers: RefCell<Subscribers>,
    pub(crate) producer: RefCell<Weak<Computation>>,
    pub(crate) freed: Cell<bool>,
    /// Whether the value is there: `None` when it is, or why not. A `Source` or async memo
    /// sets it; a memo takes it from what it read on its last run.
    pub(crate) readiness: RefCell<Option<Pending>>,
}

impl Default for Node {
    fn default() -> Self {
        Self {
            version: Cell::default(),
            tracked_by: Cell::default(),
            subscribers: RefCell::default(),
            producer: RefCell::new(no_computation()),
            freed: Cell::new(false),
            readiness: RefCell::new(None),
        }
    }
}

#[derive(Default)]
pub(crate) struct Subscribers {
    slots: Vec<Option<Weak<Computation>>>,
    free: Vec<usize>,
}

impl Subscribers {
    pub(crate) fn add(&mut self, subscriber: Weak<Computation>) -> usize {
        match self.free.pop() {
            Some(slot) => {
                self.slots[slot] = Some(subscriber);
                slot
            }
            None => {
                self.slots.push(Some(subscriber));
                self.slots.len() - 1
            }
        }
    }

    pub(crate) fn remove(&mut self, slot: usize, subscriber: *const Computation) {
        let held = self.slots.get(slot).and_then(Option::as_ref);
        if !held.is_some_and(|held| std::ptr::addr_eq(held.as_ptr(), subscriber)) {
            return;
        }
        self.slots[slot] = None;
        self.free.push(slot);
        if self.free.len() == self.slots.len() {
            self.slots.clear();
            self.free.clear();
        }
    }

    pub(crate) fn live(&self) -> Vec<Rc<Computation>> {
        self.slots
            .iter()
            .filter_map(|slot| slot.as_ref()?.upgrade())
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.slots.len() - self.free.len()
    }
}

pub(crate) trait Tracked {
    fn source(&self) -> &Node;
}

impl Tracked for Node {
    fn source(&self) -> &Node {
        self
    }
}

impl<T> Tracked for Value<T> {
    fn source(&self) -> &Node {
        &self.source
    }
}

const SCANNED_DEPENDENCIES: usize = 16;

pub(crate) fn track(tracked: Rc<dyn Tracked>) {
    let Some(observer) = RUNTIME.with(|runtime| runtime.observer.borrow().upgrade()) else {
        RUNTIME.with(|runtime| {
            if let Some(probe) = runtime.probe.borrow_mut().as_mut()
                && !probe
                    .read
                    .iter()
                    .any(|(source, _)| std::ptr::addr_eq(Rc::as_ptr(source), Rc::as_ptr(&tracked)))
            {
                let version = tracked.source().version.get();
                probe.read.push((tracked, version));
            }
        });
        return;
    };
    {
        if observer.is_disposed() {
            return;
        }
        let tracking = observer.tracking.get();
        let source = tracked.source();
        if source.tracked_by.replace(tracking) == tracking {
            return;
        }
        let mut dependencies = observer.dependencies.borrow_mut();
        let held = dependencies.len() <= SCANNED_DEPENDENCIES
            && dependencies
                .iter()
                .any(|(source, _, _)| std::ptr::addr_eq(Rc::as_ptr(source), Rc::as_ptr(&tracked)));
        if !held {
            let slot = source
                .subscribers
                .borrow_mut()
                .add(Rc::downgrade(&observer));
            let version = source.version.get();
            dependencies.push((tracked, version, slot));
        }
    }
}

impl Node {
    pub(crate) fn readiness(&self) -> Option<Pending> {
        self.readiness.borrow().clone()
    }

    pub(crate) fn is_settled(&self) -> bool {
        self.readiness.borrow().is_none()
    }

    /// Whether it changed.
    pub(crate) fn set_readiness(&self, readiness: Option<Pending>) -> bool {
        if *self.readiness.borrow() == readiness {
            return false;
        }
        self.readiness.replace(readiness);
        true
    }

    pub(crate) fn changed(&self) {
        self.version.set(self.version.get().wrapping_add(1));
        let subscribers = self.subscribers.borrow().live();
        for subscriber in subscribers {
            subscriber.invalidate(State::Dirty);
        }
    }
}

pub(crate) struct Value<T> {
    pub(crate) source: Node,
    pub(crate) value: RefCell<T>,
}

/// Reading a reactive value. Implemented by signals, memos, `Reader`, stores, `Prop` and
/// `Binding`, so a helper that only reads can take `impl Get<T>`; a field or prop that keeps
/// something to read later holds a `Reader<T>`.
///
/// The tracked reads (`get`, `with`) panic in debug builds inside a component body, which
/// runs once and would never rerun on the change (`component_body`).
pub trait Get<T> {
    /// Holds a shared borrow for the closure, so writing the same signal inside it is a
    /// borrow conflict.
    #[track_caller]
    fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R;

    fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        untrack(|| self.with(f))
    }

    #[track_caller]
    fn get(&self) -> T
    where
        T: Clone,
    {
        self.with(T::clone)
    }

    /// Subscribes the running computation without reading the value: for the dependencies
    /// of `create_effect_on`, or a memo that recomputes from something else when this
    /// changes.
    #[track_caller]
    fn track(&self) {
        self.with(|_| ());
    }

    fn get_untracked(&self) -> T
    where
        T: Clone,
    {
        self.with_untracked(T::clone)
    }
}

/// Writing a reactive value. A write is visible to reads at once; the effects it wakes
/// are queued, not run.
pub trait Set<T> {
    /// Mutates in place. A `Signal` always notifies, with no `PartialEq` needed, holds a
    /// mutable borrow (so reading the same signal inside the closure is a borrow conflict),
    /// and still notifies when the closure panics, since the value may already have changed.
    /// A `#[derive(Store)]` store edits a copy and writes each field back with `set`, so it
    /// notifies only the fields that differ, and nothing if the closure panics.
    fn update<R>(&self, f: impl FnOnce(&mut T) -> R) -> R;

    /// Notifies only when the value differs, so a caller never compares first: an equal
    /// value wakes nothing. Does nothing on a freed signal, since nothing can read the
    /// value.
    fn set(&self, value: T)
    where
        T: PartialEq;
}

/// A readable and writable source, owned by the scope it was created in. `Copy` whatever
/// `T` is, and equal to another handle when both name the same signal. Reading it after
/// that scope is disposed panics; writing it does nothing. `set` skips a value equal to the
/// one it holds; `set_unconditionally` and `update` always notify.
pub struct Signal<T> {
    key: Key,
    ty: PhantomData<*const T>,
}

/// The same signal without the writes, for a field or published state the receiver must
/// not write.
pub struct ReadSignal<T> {
    key: Key,
    ty: PhantomData<*const T>,
}

/// Panics with no current scope; outside one use `OwnedSignal` or `RootScope`.
pub fn create_signal<T: 'static>(value: T) -> Signal<T> {
    Signal::from_key(arena::insert_owned_tracked(Rc::new(Value::new(value))))
}

/// A signal freed when this value drops rather than with a scope: for a signal per entry
/// of a map that outlives its readers, or a struct field made where no scope is current.
/// Derefs to its `Signal`.
pub struct OwnedSignal<T: 'static> {
    handle: Signal<T>,
}

impl<T: 'static> OwnedSignal<T> {
    pub fn new(value: T) -> Self {
        Self {
            handle: Signal::from_key(arena::insert_tracked(Rc::new(Value::new(value)))),
        }
    }

    pub fn handle(&self) -> Signal<T> {
        self.handle
    }
}

impl<T: 'static> std::ops::Deref for OwnedSignal<T> {
    type Target = Signal<T>;

    fn deref(&self) -> &Signal<T> {
        &self.handle
    }
}

impl<T: 'static> Drop for OwnedSignal<T> {
    fn drop(&mut self) {
        arena::remove(self.handle.key);
    }
}

impl<T> Value<T> {
    pub(crate) fn new(value: T) -> Self {
        Self {
            value: RefCell::new(value),
            source: Node::default(),
        }
    }
}

fn stored<T: 'static>(key: Key) -> Rc<Value<T>> {
    arena::get(key).expect("signal used after the scope that owns it was disposed")
}

fn with<T: 'static, R>(key: Key, f: impl FnOnce(&T) -> R) -> R {
    let inner = stored::<T>(key);
    track(inner.clone());
    f(&inner.value.borrow())
}

fn with_untracked<T: 'static, R>(key: Key, f: impl FnOnce(&T) -> R) -> R {
    f(&stored::<T>(key).value.borrow())
}

fn is_alive<T: 'static>(key: Key) -> bool {
    arena::get::<Value<T>>(key).is_some()
}

impl<T> Clone for Signal<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Signal<T> {}

impl<T> Clone for ReadSignal<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for ReadSignal<T> {}

/// Two handles are equal when they name the same signal.
impl<T> PartialEq for Signal<T> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl<T> Eq for Signal<T> {}

impl<T> PartialEq for ReadSignal<T> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl<T> Eq for ReadSignal<T> {}

impl<T> std::fmt::Debug for Signal<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Signal").field(&self.key).finish()
    }
}

impl<T> std::fmt::Debug for ReadSignal<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ReadSignal").field(&self.key).finish()
    }
}

impl<T: 'static> Get<T> for Signal<T> {
    fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        check_tracked_read::<Self>();
        with(self.key, f)
    }

    fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        with_untracked(self.key, f)
    }
}

impl<T: 'static> Get<T> for ReadSignal<T> {
    fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        check_tracked_read::<Self>();
        with(self.key, f)
    }

    fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        with_untracked(self.key, f)
    }
}

impl<T: 'static> Set<T> for Signal<T> {
    fn set(&self, value: T)
    where
        T: PartialEq,
    {
        assert_writable();
        let Some(inner) = arena::get::<Value<T>>(self.key) else {
            return;
        };
        let mut current = inner.value.borrow_mut();
        if *current != value {
            *current = value;
            drop(current);
            inner.source.changed();
        }
    }

    fn update<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        assert_writable();
        let inner = stored::<T>(self.key);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            f(&mut inner.value.borrow_mut())
        }));
        inner.source.changed();
        match result {
            Ok(value) => value,
            Err(error) => std::panic::resume_unwind(error),
        }
    }
}

impl<T: 'static> Signal<T> {
    fn from_key(key: Key) -> Self {
        Self {
            key,
            ty: PhantomData,
        }
    }

    pub fn read_only(&self) -> ReadSignal<T> {
        ReadSignal {
            key: self.key,
            ty: PhantomData,
        }
    }

    pub fn set_unconditionally(&self, value: T) {
        if self.is_alive() {
            self.update(|current| *current = value);
        }
    }

    pub fn is_alive(&self) -> bool {
        is_alive::<T>(self.key)
    }

    #[cfg(test)]
    pub(crate) fn subscribers(&self) -> usize {
        stored::<T>(self.key).source.subscribers.borrow().len()
    }
}

impl<T: 'static> ReadSignal<T> {
    pub fn is_alive(&self) -> bool {
        is_alive::<T>(self.key)
    }
}

pub(crate) fn assert_writable() {
    assert!(
        RUNTIME.with(|runtime| runtime.memo_depth.get() == 0),
        "memo computations must not write signals"
    );
}

/// What a value that read `nodes` is: failed if any failed, else loading if any is.
pub(crate) fn readiness_of<'a>(nodes: impl Iterator<Item = &'a Node>) -> Option<Pending> {
    let mut readiness = None;
    for node in nodes {
        match &*node.readiness.borrow() {
            None => {}
            Some(Pending::Failed(error)) => return Some(Pending::Failed(error.clone())),
            Some(Pending::Loading) => readiness = Some(Pending::Loading),
        }
    }
    readiness
}

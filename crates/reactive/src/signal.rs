use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::rc::{Rc, Weak};

use crate::arena::{self, Key};

use crate::computation::{Computation, State, no_computation};
use crate::runtime::{RUNTIME, batch};

pub(crate) struct Source {
    pub(crate) version: Cell<u64>,
    pub(crate) subscribers: RefCell<Vec<Weak<Computation>>>,
    pub(crate) producer: RefCell<Weak<Computation>>,
}

impl Default for Source {
    fn default() -> Self {
        Self {
            version: Cell::default(),
            subscribers: RefCell::default(),
            producer: RefCell::new(no_computation()),
        }
    }
}

pub(crate) trait Tracked {
    fn source(&self) -> &Source;
}

impl Tracked for Source {
    fn source(&self) -> &Source {
        self
    }
}

impl<T> Tracked for Value<T> {
    fn source(&self) -> &Source {
        &self.source
    }
}

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
        let mut dependencies = observer.dependencies.borrow_mut();
        if !dependencies
            .iter()
            .any(|(source, _)| std::ptr::addr_eq(Rc::as_ptr(source), Rc::as_ptr(&tracked)))
        {
            let source = tracked.source();
            source
                .subscribers
                .borrow_mut()
                .push(Rc::downgrade(&observer));
            let version = source.version.get();
            dependencies.push((tracked, version));
        }
    }
}

impl Source {
    pub(crate) fn changed(&self) {
        self.version.set(self.version.get().wrapping_add(1));
        let subscribers = self.subscribers.borrow().clone();
        for subscriber in subscribers {
            if let Some(subscriber) = subscriber.upgrade() {
                subscriber.invalidate(State::Dirty);
            }
        }
    }
}

pub(crate) struct Value<T> {
    pub(crate) source: Source,
    pub(crate) value: RefCell<T>,
}

pub struct ReadSignal<T> {
    key: Key,
    ty: PhantomData<*const T>,
}

pub struct WriteSignal<T> {
    key: Key,
    ty: PhantomData<*const T>,
}

pub fn create_signal<T: 'static>(value: T) -> (ReadSignal<T>, WriteSignal<T>) {
    handles(arena::insert_owned(Rc::new(Value::new(value))))
}

pub struct OwnedSignal<T: 'static> {
    read: ReadSignal<T>,
    write: WriteSignal<T>,
}

impl<T: 'static> OwnedSignal<T> {
    pub fn new(value: T) -> Self {
        let (read, write) = handles(arena::insert(Rc::new(Value::new(value))));
        Self { read, write }
    }

    pub fn read(&self) -> ReadSignal<T> {
        self.read
    }

    pub fn write(&self) -> WriteSignal<T> {
        self.write
    }
}

impl<T: 'static> Drop for OwnedSignal<T> {
    fn drop(&mut self) {
        arena::remove(self.read.key);
    }
}

fn handles<T>(key: Key) -> (ReadSignal<T>, WriteSignal<T>) {
    (
        ReadSignal {
            key,
            ty: PhantomData,
        },
        WriteSignal {
            key,
            ty: PhantomData,
        },
    )
}

impl<T> Value<T> {
    pub(crate) fn new(value: T) -> Self {
        Self {
            value: RefCell::new(value),
            source: Source::default(),
        }
    }
}

fn stored<T: 'static>(key: Key) -> Rc<Value<T>> {
    arena::get(key).expect("signal used after the scope that owns it was disposed")
}

impl<T> Clone for ReadSignal<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for ReadSignal<T> {}

impl<T> Clone for WriteSignal<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for WriteSignal<T> {}

impl<T: 'static> ReadSignal<T> {
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        let inner = stored::<T>(self.key);
        track(inner.clone());
        f(&inner.value.borrow())
    }

    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.with(Clone::clone)
    }

    pub fn get_untracked(&self) -> T
    where
        T: Clone,
    {
        self.with_untracked(Clone::clone)
    }

    pub fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        f(&stored::<T>(self.key).value.borrow())
    }

    pub fn is_alive(&self) -> bool {
        arena::get::<Value<T>>(self.key).is_some()
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        self.key == other.key
    }

    #[cfg(test)]
    pub(crate) fn subscribers(&self) -> usize {
        stored::<T>(self.key).source.subscribers.borrow().len()
    }
}

impl<T: 'static> WriteSignal<T> {
    pub fn set(&self, value: T)
    where
        T: PartialEq,
    {
        assert_writable();
        let Some(inner) = arena::get::<Value<T>>(self.key) else {
            return;
        };
        batch(|| {
            let mut current = inner.value.borrow_mut();
            if *current != value {
                *current = value;
                drop(current);
                inner.source.changed();
            }
        });
    }

    pub fn update<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        assert_writable();
        let inner = stored::<T>(self.key);
        batch(|| {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                f(&mut inner.value.borrow_mut())
            }));
            inner.source.changed();
            match result {
                Ok(value) => value,
                Err(error) => std::panic::resume_unwind(error),
            }
        })
    }

    pub fn set_unconditionally(&self, value: T) {
        if self.is_alive() {
            self.update(|current| *current = value);
        }
    }

    pub fn is_alive(&self) -> bool {
        arena::get::<Value<T>>(self.key).is_some()
    }
}

fn assert_writable() {
    assert!(
        RUNTIME.with(|runtime| runtime.memo_depth.get() == 0),
        "memo computations must not write signals"
    );
}

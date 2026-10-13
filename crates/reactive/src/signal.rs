use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::rc::{Rc, Weak};

use crate::arena::{self, Key};

use crate::computation::{Computation, State};
use crate::runtime::{RUNTIME, batch};

#[derive(Default)]
pub(crate) struct Source {
    pub(crate) version: Cell<u64>,
    pub(crate) subscribers: RefCell<Vec<Weak<Computation>>>,
    pub(crate) producer: RefCell<Weak<Computation>>,
}

impl Source {
    pub(crate) fn track(self: &Rc<Self>) {
        if let Some(observer) = RUNTIME.with(|runtime| runtime.observer.borrow().upgrade()) {
            if observer.is_disposed() {
                return;
            }
            let mut dependencies = observer.dependencies.borrow_mut();
            if !dependencies
                .iter()
                .any(|(source, _)| Rc::ptr_eq(source, self))
            {
                dependencies.push((self.clone(), self.version.get()));
                self.subscribers.borrow_mut().push(Rc::downgrade(&observer));
            }
        }
    }

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
    pub(crate) value: RefCell<T>,
    pub(crate) source: Rc<Source>,
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
            source: Rc::new(Source::default()),
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
        inner.source.track();
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
    pub(crate) fn source(&self) -> Rc<Source> {
        stored::<T>(self.key).source.clone()
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

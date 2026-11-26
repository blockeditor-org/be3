use std::marker::PhantomData;
use std::rc::Rc;

use crate::arena::{self, Key};
use crate::loading::{Pending, Ready, note_read};
use crate::runtime::check_tracked_read;
use crate::signal::{Get, Node, Set, Tracked, Value, assert_writable, track};

/// Data that arrives later, settled from outside the graph: `Err(Pending::Loading)` until
/// whoever holds it sets the value (a host callback, an answer to a request). Like an async
/// memo's, its readiness is part of the graph: a memo that reads it is pending while it is,
/// and a read of it outside a memo waits at the nearest `Loading` (or fails to the nearest
/// `Errored`). Data that arrives later is a `Source` or an async memo, never a
/// `Signal<Ready<T>>`, whose reads the boundaries would not see.
pub struct Source<T> {
    key: Key,
    ty: PhantomData<*const T>,
}

/// Panics with no current scope; outside one use `OwnedSource`.
pub fn create_source<T: 'static>() -> Source<T> {
    Source::from_key(arena::insert_owned_tracked(loading_value::<T>()))
}

/// A `Source` freed when this value drops rather than with a scope. Derefs to its `Source`.
pub struct OwnedSource<T: 'static> {
    handle: Source<T>,
}

impl<T: 'static> OwnedSource<T> {
    pub fn new() -> Self {
        Self {
            handle: Source::from_key(arena::insert_tracked(loading_value::<T>())),
        }
    }

    pub fn handle(&self) -> Source<T> {
        self.handle
    }
}

/// A source that starts as `value` rather than loading: one made where nothing may be
/// written, such as inside a memo, whose value is already known.
impl<T: 'static> From<Ready<T>> for OwnedSource<T> {
    fn from(value: Ready<T>) -> Self {
        let stored = Rc::new(Value::new(value));
        stored
            .source
            .set_readiness(readiness(&stored.value.borrow()));
        Self {
            handle: Source::from_key(arena::insert_tracked(stored)),
        }
    }
}

impl<T: 'static> Default for OwnedSource<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: 'static> std::ops::Deref for OwnedSource<T> {
    type Target = Source<T>;

    fn deref(&self) -> &Source<T> {
        &self.handle
    }
}

impl<T: 'static> Drop for OwnedSource<T> {
    fn drop(&mut self) {
        arena::remove(self.handle.key);
    }
}

fn loading_value<T>() -> Rc<Value<Ready<T>>> {
    let value = Rc::new(Value::new(Err(Pending::Loading)));
    value.source.set_readiness(Some(Pending::Loading));
    value
}

fn readiness<T>(value: &Ready<T>) -> Option<Pending> {
    value.as_ref().err().cloned()
}

impl<T> Clone for Source<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Source<T> {}

impl<T> PartialEq for Source<T> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl<T> Eq for Source<T> {}

impl<T> std::fmt::Debug for Source<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Source").field(&self.key).finish()
    }
}

impl<T: 'static> Source<T> {
    fn from_key(key: Key) -> Self {
        Self {
            key,
            ty: PhantomData,
        }
    }

    fn stored(&self) -> Rc<Value<Ready<T>>> {
        arena::get(self.key).expect("source used after the scope that owns it was disposed")
    }

    pub fn is_alive(&self) -> bool {
        arena::get::<Value<Ready<T>>>(self.key).is_some()
    }

    pub(crate) fn with_node<R>(&self, f: impl FnOnce(&Node) -> R) -> R {
        f(&self.stored().source)
    }

    fn read<R>(&self, tracked: bool, f: impl FnOnce(&Ready<T>) -> R) -> R {
        let inner = self.stored();
        if !inner.source.is_settled() {
            note_read(inner.clone() as Rc<dyn Tracked>);
        }
        if tracked {
            track(inner.clone());
        }
        f(&inner.value.borrow())
    }
}

impl<T: 'static> Get<Ready<T>> for Source<T> {
    #[track_caller]
    fn with<R>(&self, f: impl FnOnce(&Ready<T>) -> R) -> R {
        check_tracked_read::<Self>();
        self.read(true, f)
    }

    fn with_untracked<R>(&self, f: impl FnOnce(&Ready<T>) -> R) -> R {
        self.read(false, f)
    }
}

impl<T: 'static> Set<Ready<T>> for Source<T> {
    /// Notifies when the value differs, or when only whether it is there does.
    fn set(&self, value: Ready<T>)
    where
        Ready<T>: PartialEq,
    {
        assert_writable();
        let Some(inner) = arena::get::<Value<Ready<T>>>(self.key) else {
            return;
        };
        let moved = inner.source.set_readiness(readiness(&value));
        let mut current = inner.value.borrow_mut();
        if moved || *current != value {
            *current = value;
            drop(current);
            inner.source.changed();
        }
    }

    fn update<R>(&self, f: impl FnOnce(&mut Ready<T>) -> R) -> R {
        assert_writable();
        let inner = self.stored();
        let result = f(&mut inner.value.borrow_mut());
        inner.source.set_readiness(readiness(&inner.value.borrow()));
        inner.source.changed();
        result
    }
}

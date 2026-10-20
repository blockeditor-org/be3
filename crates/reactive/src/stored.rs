use std::cell::Cell;
use std::marker::PhantomData;
use std::rc::Rc;

use crate::arena::{self, Key};

pub struct Stored<T> {
    key: Key,
    ty: PhantomData<*const T>,
}

pub fn store<T: 'static>(value: T) -> Stored<T> {
    Stored {
        key: arena::insert_owned(Rc::new(value)),
        ty: PhantomData,
    }
}

impl<T> Clone for Stored<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Stored<T> {}

impl<T> PartialEq for Stored<T> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl<T> Eq for Stored<T> {}

impl<T> std::fmt::Debug for Stored<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Stored").field(&self.key).finish()
    }
}

impl<T: 'static> Stored<T> {
    pub fn value(&self) -> Rc<T> {
        self.try_value()
            .expect("stored value used after the scope that owns it was disposed")
    }

    pub fn try_value(&self) -> Option<Rc<T>> {
        arena::get(self.key)
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        f(&self.value())
    }

    pub fn try_with<R>(&self, f: impl FnOnce(&T) -> R) -> Option<R> {
        self.try_value().map(|value| f(&value))
    }

    pub fn is_alive(&self) -> bool {
        self.try_value().is_some()
    }
}

pub struct OwnedStored<T: 'static> {
    handle: Stored<T>,
}

impl<T: 'static> OwnedStored<T> {
    pub fn new(value: T) -> Self {
        Self {
            handle: Stored {
                key: arena::insert(Rc::new(value)),
                ty: PhantomData,
            },
        }
    }

    pub fn handle(&self) -> Stored<T> {
        self.handle
    }
}

impl<T: 'static> std::ops::Deref for OwnedStored<T> {
    type Target = Stored<T>;

    fn deref(&self) -> &Stored<T> {
        &self.handle
    }
}

impl<T: 'static> Drop for OwnedStored<T> {
    fn drop(&mut self) {
        arena::remove(self.handle.key);
    }
}

pub struct StoredCell<T>(Stored<Cell<T>>);

impl<T> Clone for StoredCell<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for StoredCell<T> {}

impl<T> PartialEq for StoredCell<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T> Eq for StoredCell<T> {}

impl<T> std::fmt::Debug for StoredCell<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("StoredCell").field(&self.0).finish()
    }
}

impl<T: Default + 'static> Default for StoredCell<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: 'static> StoredCell<T> {
    pub fn new(value: T) -> Self {
        Self(store(Cell::new(value)))
    }

    pub fn get(&self) -> T
    where
        T: Copy,
    {
        self.0.with(Cell::get)
    }

    pub fn set(&self, value: T) {
        self.0.try_with(|cell| cell.set(value));
    }

    pub fn replace(&self, value: T) -> T {
        self.0.with(|cell| cell.replace(value))
    }

    pub fn take(&self) -> T
    where
        T: Default,
    {
        self.0.with(Cell::take)
    }

    pub fn update(&self, f: impl FnOnce(T) -> T)
    where
        T: Copy,
    {
        self.0.try_with(|cell| cell.set(f(cell.get())));
    }

    pub fn is_alive(&self) -> bool {
        self.0.is_alive()
    }
}

use std::cell::{BorrowError, BorrowMutError, Ref, RefCell, RefMut};
use std::ops::{Deref, DerefMut};

use beui_view::reactive::*;

pub(crate) struct Watched<T, S: Copy + PartialEq + 'static = bool> {
    value: RefCell<T>,
    flag: Signal<S>,
    test: fn(&T) -> S,
}

pub(crate) struct WatchedMut<'a, T, S: Copy + PartialEq + 'static = bool> {
    value: Option<RefMut<'a, T>>,
    flag: Signal<S>,
    test: fn(&T) -> S,
}

impl<T, S: Copy + PartialEq + 'static> Watched<T, S> {
    pub(crate) fn new(value: T, test: fn(&T) -> S) -> Self {
        let flag = create_signal(test(&value));
        Self {
            value: RefCell::new(value),
            flag,
            test,
        }
    }

    pub(crate) fn flag(&self) -> ReadSignal<S> {
        self.flag.read_only()
    }

    pub(crate) fn borrow(&self) -> Ref<'_, T> {
        self.value.borrow()
    }

    pub(crate) fn try_borrow(&self) -> Result<Ref<'_, T>, BorrowError> {
        self.value.try_borrow()
    }

    pub(crate) fn borrow_mut(&self) -> WatchedMut<'_, T, S> {
        WatchedMut {
            value: Some(self.value.borrow_mut()),
            flag: self.flag,
            test: self.test,
        }
    }

    pub(crate) fn try_borrow_mut(&self) -> Result<WatchedMut<'_, T, S>, BorrowMutError> {
        Ok(WatchedMut {
            value: Some(self.value.try_borrow_mut()?),
            flag: self.flag,
            test: self.test,
        })
    }
}

impl<T, S: Copy + PartialEq + 'static> Deref for WatchedMut<'_, T, S> {
    type Target = T;

    fn deref(&self) -> &T {
        self.value.as_deref().expect("held until dropped")
    }
}

impl<T, S: Copy + PartialEq + 'static> DerefMut for WatchedMut<'_, T, S> {
    fn deref_mut(&mut self) -> &mut T {
        self.value.as_deref_mut().expect("held until dropped")
    }
}

impl<T, S: Copy + PartialEq + 'static> Drop for WatchedMut<'_, T, S> {
    fn drop(&mut self) {
        let test = self.test;
        if let Some(value) = self.value.take() {
            let now = test(&value);
            drop(value);
            if self.flag.get_untracked() != now {
                self.flag.set(now);
            }
        }
    }
}

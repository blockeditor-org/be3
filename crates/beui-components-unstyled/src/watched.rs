use std::cell::{BorrowError, BorrowMutError, Ref, RefCell, RefMut};
use std::ops::{Deref, DerefMut};

use beui_view::reactive::*;

pub(crate) struct Watched<T> {
    value: RefCell<T>,
    flag: Signal<bool>,
    test: fn(&T) -> bool,
}

pub(crate) struct WatchedMut<'a, T> {
    value: Option<RefMut<'a, T>>,
    flag: Signal<bool>,
    test: fn(&T) -> bool,
}

impl<T> Watched<T> {
    pub(crate) fn new(value: T, test: fn(&T) -> bool) -> Self {
        let flag = create_signal(test(&value));
        Self {
            value: RefCell::new(value),
            flag,
            test,
        }
    }

    pub(crate) fn flag(&self) -> ReadSignal<bool> {
        self.flag.read_only()
    }

    pub(crate) fn borrow(&self) -> Ref<'_, T> {
        self.value.borrow()
    }

    pub(crate) fn try_borrow(&self) -> Result<Ref<'_, T>, BorrowError> {
        self.value.try_borrow()
    }

    pub(crate) fn borrow_mut(&self) -> WatchedMut<'_, T> {
        WatchedMut {
            value: Some(self.value.borrow_mut()),
            flag: self.flag,
            test: self.test,
        }
    }

    pub(crate) fn try_borrow_mut(&self) -> Result<WatchedMut<'_, T>, BorrowMutError> {
        Ok(WatchedMut {
            value: Some(self.value.try_borrow_mut()?),
            flag: self.flag,
            test: self.test,
        })
    }
}

impl<T> Deref for WatchedMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.value.as_deref().expect("held until dropped")
    }
}

impl<T> DerefMut for WatchedMut<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.value.as_deref_mut().expect("held until dropped")
    }
}

impl<T> Drop for WatchedMut<'_, T> {
    fn drop(&mut self) {
        let test = self.test;
        let now = self.value.take().is_some_and(|value| test(&value));
        if self.flag.get_untracked() != now {
            self.flag.set(now);
        }
    }
}

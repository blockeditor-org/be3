use std::cell::RefCell;

use reactive::{Stored, store, untrack};

type Handler<V, R> = Box<dyn FnMut(V) -> R>;
type ClickHandler = Box<dyn FnMut()>;
type Slot<H> = Option<Stored<RefCell<Option<H>>>>;

pub struct Callback<V, R = ()>(Slot<Handler<V, R>>);

impl<V: 'static, R: 'static> Callback<V, R> {
    pub fn new(handler: impl FnMut(V) -> R + 'static) -> Self {
        Self(Some(store(RefCell::new(Some(Box::new(handler))))))
    }

    pub fn empty() -> Self {
        Self(None)
    }

    pub fn is_empty(&self) -> bool {
        self.0
            .and_then(|slot| slot.try_with(|slot| slot.borrow().is_none()))
            .unwrap_or(true)
    }

    pub fn call(&self, value: V) -> R
    where
        R: Default,
    {
        let Some(slot) = self.0.and_then(|slot| slot.try_value()) else {
            return R::default();
        };
        let Some(mut handler) = slot.borrow_mut().take() else {
            return R::default();
        };
        let result = untrack(|| handler(value));
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = Some(handler);
        }
        result
    }
}

impl<V, R> Clone for Callback<V, R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V, R> Copy for Callback<V, R> {}

impl<V, R> Default for Callback<V, R> {
    fn default() -> Self {
        Self(None)
    }
}

#[derive(Clone, Copy, Default)]
pub struct ClickCallback(Slot<ClickHandler>);

impl ClickCallback {
    pub fn new(handler: impl FnMut() + 'static) -> Self {
        Self(Some(store(RefCell::new(Some(Box::new(handler))))))
    }

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.0
            .and_then(|slot| slot.try_with(|slot| slot.borrow().is_none()))
            .unwrap_or(true)
    }

    pub fn call(&self) {
        let Some(slot) = self.0.and_then(|slot| slot.try_value()) else {
            return;
        };
        let Some(mut handler) = slot.borrow_mut().take() else {
            return;
        };
        untrack(&mut handler);
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = Some(handler);
        }
    }
}

pub trait IntoCallback<V, R> {
    fn into_callback(self) -> Callback<V, R>;
}

impl<V: 'static, R: 'static, F: FnMut(V) -> R + 'static> IntoCallback<V, R> for F {
    fn into_callback(self) -> Callback<V, R> {
        Callback::new(self)
    }
}

impl<V, R> IntoCallback<V, R> for Callback<V, R> {
    fn into_callback(self) -> Callback<V, R> {
        self
    }
}

pub trait IntoClickCallback {
    fn into_click_callback(self) -> ClickCallback;
}

impl<F: FnMut() + 'static> IntoClickCallback for F {
    fn into_click_callback(self) -> ClickCallback {
        ClickCallback::new(self)
    }
}

impl IntoClickCallback for ClickCallback {
    fn into_click_callback(self) -> ClickCallback {
        self
    }
}

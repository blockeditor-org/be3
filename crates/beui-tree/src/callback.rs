use std::cell::RefCell;

use beui_reactive::{Set, Signal, Stored, store, untrack};

type Handler<V, R> = Box<dyn FnMut(V) -> R>;

pub struct Callback<V, R = ()>(Target<V, R>);

enum Target<V, R> {
    Empty,
    Handler(Stored<RefCell<Option<Handler<V, R>>>>),
    Signal(Signal<V>, fn(&Signal<V>, V) -> R),
}

impl<V, R> Clone for Target<V, R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V, R> Copy for Target<V, R> {}

impl<V: 'static, R: 'static> Callback<V, R> {
    pub fn new(handler: impl FnMut(V) -> R + 'static) -> Self {
        Self(Target::Handler(store(RefCell::new(Some(Box::new(
            handler,
        ))))))
    }

    pub fn empty() -> Self {
        Self(Target::Empty)
    }

    pub fn is_empty(&self) -> bool {
        match self.0 {
            Target::Empty => true,
            Target::Handler(slot) => slot
                .try_with(|slot| slot.borrow().is_none())
                .unwrap_or(true),
            Target::Signal(signal, _) => !signal.is_alive(),
        }
    }

    pub fn call(&self, value: V) -> R
    where
        R: Default,
    {
        let slot = match self.0 {
            Target::Empty => return R::default(),
            Target::Signal(signal, set) => {
                if !signal.is_alive() {
                    return R::default();
                }
                return untrack(|| set(&signal, value));
            }
            Target::Handler(slot) => slot,
        };
        let Some(slot) = slot.try_value() else {
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
        Self(Target::Empty)
    }
}

impl<V: PartialEq + 'static> From<Signal<V>> for Callback<V> {
    fn from(signal: Signal<V>) -> Self {
        Self(Target::Signal(signal, <Signal<V> as Set<V>>::set))
    }
}

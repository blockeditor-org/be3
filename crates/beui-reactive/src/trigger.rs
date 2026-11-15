use std::rc::Rc;

use crate::memo::Memo;
use crate::signal::{Get, ReadSignal, Set, Signal, create_signal};

/// A signal with no value: `notify` wakes whatever `track`ed it. For an event
/// to react to ("the list moved") where a counter would be kept only to be bumped.
/// Owned by the scope it was made in, like a `Signal`.
#[derive(Clone, Copy)]
pub struct Trigger(Signal<()>);

/// Panics with no current scope, like `create_signal`.
pub fn create_trigger() -> Trigger {
    Trigger(create_signal(()))
}

impl Trigger {
    /// Wakes everything that tracked this trigger. Does nothing once its scope is gone.
    pub fn notify(self) {
        if self.0.is_alive() {
            self.0.update(|()| ());
        }
    }

    /// Makes the current computation rerun on the next `notify`.
    #[track_caller]
    pub fn track(self) {
        self.0.with(|()| ());
    }
}

/// Anything a computation can track for its changes alone, whatever it holds: a
/// `Signal`, `ReadSignal`, `Memo` or `Trigger`, or `Trackable::new` for anything else.
/// A prop that only asks "when did this change" takes one; each of those converts
/// into it, and into `Option<Trackable>` for an optional prop.
#[derive(Clone)]
pub struct Trackable(Rc<dyn Fn()>);

impl Trackable {
    pub fn new(track: impl Fn() + 'static) -> Self {
        Self(Rc::new(track))
    }

    /// Makes the current computation rerun when what this follows changes.
    pub fn track(&self) {
        (self.0)();
    }
}

macro_rules! trackable {
    ($($ty:ident<$t:ident>),*) => {$(
        impl<$t: 'static> From<$ty<$t>> for Trackable {
            fn from(source: $ty<$t>) -> Self {
                Trackable::new(move || source.with(|_| ()))
            }
        }

        impl<$t: 'static> From<$ty<$t>> for Option<Trackable> {
            fn from(source: $ty<$t>) -> Self {
                Some(source.into())
            }
        }
    )*};
}

trackable!(Signal<T>, ReadSignal<T>, Memo<T>);

impl From<Trigger> for Trackable {
    fn from(trigger: Trigger) -> Self {
        Trackable::new(move || trigger.track())
    }
}

impl From<Trigger> for Option<Trackable> {
    fn from(trigger: Trigger) -> Self {
        Some(trigger.into())
    }
}

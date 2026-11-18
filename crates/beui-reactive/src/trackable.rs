use crate::memo::Memo;
use crate::reader::Reader;
use crate::signal::{Get, ReadSignal, Signal};
use crate::stored::{Stored, store};
use crate::trigger::Trigger;

/// Anything a computation can track for its changes alone, whatever it holds: a
/// `Trigger`, or a `Signal`, `ReadSignal`, `Memo` or `Reader` of any type. A prop that
/// only asks "when did this change" (`Glide`'s `on`) takes one, so its caller passes an
/// event or a value alike; each converts into it, and into `Option<Trackable>` for an
/// optional prop. `Trackable::new` covers anything else. `Copy`, like every handle: what
/// it tracks is kept in the scope that made it.
pub struct Trackable(Stored<Box<dyn Fn()>>);

impl Clone for Trackable {
    fn clone(&self) -> Self {
        *self
    }
}

impl Copy for Trackable {}

impl Trackable {
    /// Panics with no current scope, like `store`.
    pub fn new(track: impl Fn() + 'static) -> Self {
        Self(store(Box::new(track)))
    }

    /// Subscribes the running computation to what this follows.
    #[track_caller]
    pub fn track(&self) {
        self.0.with(|track| track());
    }
}

macro_rules! trackable {
    ($($ty:ident<$t:ident>),*) => {$(
        impl<$t: 'static> From<$ty<$t>> for Trackable {
            fn from(source: $ty<$t>) -> Self {
                Trackable::new(move || source.track())
            }
        }

        impl<$t: 'static> From<$ty<$t>> for Option<Trackable> {
            fn from(source: $ty<$t>) -> Self {
                Some(source.into())
            }
        }
    )*};
}

trackable!(Signal<T>, ReadSignal<T>, Memo<T>, Reader<T>);

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

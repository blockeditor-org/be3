use std::rc::Rc;

use crate::memo::Memo;
use crate::reader::Reader;
use crate::signal::{Get, ReadSignal, Signal};
use crate::trigger::Trigger;

/// Anything a computation can track for its changes alone, whatever it holds: a
/// `Trigger`, or a `Signal`, `ReadSignal`, `Memo` or `Reader` of any type. A prop that
/// only asks "when did this change" (`Glide`'s `on`) takes one, so its caller passes an
/// event or a value alike; each converts into it, and into `Option<Trackable>` for an
/// optional prop. `Trackable::new` covers anything else.
#[derive(Clone)]
pub struct Trackable(Rc<dyn Fn()>);

impl Trackable {
    pub fn new(track: impl Fn() + 'static) -> Self {
        Self(Rc::new(track))
    }

    /// Subscribes the running computation to what this follows.
    #[track_caller]
    pub fn track(&self) {
        (self.0)();
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

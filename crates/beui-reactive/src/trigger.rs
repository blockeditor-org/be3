use std::rc::Rc;

use crate::arena::{self, Key};
use crate::runtime::check_tracked_read;
use crate::signal::{Source, assert_writable, track};

/// An event with no value: `notify` wakes whatever `track`ed it, as a write would. For
/// something the graph cannot see changing (a request to focus, a buffer filled in
/// place); where there is a value, make it a signal instead, and readers wake on what
/// changed rather than on every event.
pub struct Trigger {
    key: Key,
}

pub fn create_trigger() -> Trigger {
    Trigger {
        key: arena::insert_owned_tracked(Rc::new(Source::default())),
    }
}

impl Clone for Trigger {
    fn clone(&self) -> Self {
        *self
    }
}

impl Copy for Trigger {}

impl PartialEq for Trigger {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Eq for Trigger {}

impl std::fmt::Debug for Trigger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Trigger").field(&self.key).finish()
    }
}

impl Trigger {
    /// Subscribes the running computation, as `get` would. Panics once the scope that made
    /// it is disposed.
    #[track_caller]
    pub fn track(&self) {
        check_tracked_read::<Self>();
        track(self.source());
    }

    /// Wakes everything that tracked it. Does nothing once its scope is disposed.
    pub fn notify(&self) {
        assert_writable();
        if let Some(source) = arena::get::<Source>(self.key) {
            source.changed();
        }
    }

    pub fn is_alive(&self) -> bool {
        arena::get::<Source>(self.key).is_some()
    }

    fn source(&self) -> Rc<Source> {
        arena::get(self.key).expect("trigger used after the scope that owns it was disposed")
    }
}

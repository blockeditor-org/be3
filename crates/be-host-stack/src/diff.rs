//! What changed in a `Seen` since the last one the worker handed over, as data
//! (plans/web_page_never_waits.md): the page applies it to its copy of that last one.
//!
//! A persistent map or set sends the entries added, changed and removed, by key. Its diff walks
//! the two versions' shared trees (`imbl::OrdMap::diff`), skipping every subtree they still
//! share, so it costs what changed rather than the size of the map, and needs no record of
//! which keys a write touched that a forgotten mark could leave out. A `Part` the worker did not
//! write since is the same `Arc` in both and sends nothing. Anything else is sent whole when it
//! is not equal to what was sent.

use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// A value the worker hands over by what changed in it.
pub(crate) trait Diff {
    type Delta: Serialize + DeserializeOwned;

    /// What makes `old` into `new`, or `None` when they are the same.
    fn diff(old: &Self, new: &Self) -> Option<Self::Delta>;

    fn apply(&mut self, delta: Self::Delta);
}

/// A value sent whole when it changed: one small enough to send again.
pub(crate) trait Whole: Clone + PartialEq + Serialize + DeserializeOwned {}

impl<T: Whole> Diff for T {
    type Delta = T;

    fn diff(old: &Self, new: &Self) -> Option<T> {
        (old != new).then(|| new.clone())
    }

    fn apply(&mut self, delta: T) {
        *self = delta;
    }
}

/// Marks each type as sent whole when it changed (`Whole`).
macro_rules! whole {
    ($($ty:ty),* $(,)?) => {
        $(impl $crate::diff::Whole for $ty {})*
    };
}

pub(crate) use whole;

whole!(
    u32,
    u64,
    usize,
    bool,
    Option<String>,
    Vec<uuid::Uuid>,
    Vec<String>,
    Option<crate::seen::Words>,
);

/// The entries of a map that were added or changed, and the keys removed.
#[derive(Deserialize, Serialize)]
pub(crate) struct MapDelta<K, V> {
    put: Vec<(K, V)>,
    removed: Vec<K>,
}

impl<K, V> Diff for imbl::OrdMap<K, V>
where
    K: Ord + Clone + Serialize + DeserializeOwned,
    V: Clone + PartialEq + Serialize + DeserializeOwned,
{
    type Delta = MapDelta<K, V>;

    fn diff(old: &Self, new: &Self) -> Option<Self::Delta> {
        if old.ptr_eq(new) {
            return None;
        }
        let mut delta = MapDelta {
            put: Vec::new(),
            removed: Vec::new(),
        };
        for item in old.diff(new) {
            match item {
                imbl::ordmap::DiffItem::Add(key, value)
                | imbl::ordmap::DiffItem::Update {
                    new: (key, value), ..
                } => delta.put.push((key.clone(), value.clone())),
                imbl::ordmap::DiffItem::Remove(key, _) => delta.removed.push(key.clone()),
            }
        }
        (!delta.put.is_empty() || !delta.removed.is_empty()).then_some(delta)
    }

    fn apply(&mut self, delta: Self::Delta) {
        for key in delta.removed {
            self.remove(&key);
        }
        for (key, value) in delta.put {
            self.insert(key, value);
        }
    }
}

/// The members of a set that were added, and those removed.
#[derive(Deserialize, Serialize)]
pub(crate) struct SetDelta<T> {
    added: Vec<T>,
    removed: Vec<T>,
}

impl<T> Diff for imbl::OrdSet<T>
where
    T: Ord + Clone + Serialize + DeserializeOwned,
{
    type Delta = SetDelta<T>;

    fn diff(old: &Self, new: &Self) -> Option<Self::Delta> {
        if old.ptr_eq(new) {
            return None;
        }
        let mut delta = SetDelta {
            added: Vec::new(),
            removed: Vec::new(),
        };
        for item in old.diff(new) {
            match item {
                imbl::ordset::DiffItem::Add(member)
                | imbl::ordset::DiffItem::Update { new: member, .. } => {
                    delta.added.push(member.clone());
                }
                imbl::ordset::DiffItem::Remove(member) => delta.removed.push(member.clone()),
            }
        }
        (!delta.added.is_empty() || !delta.removed.is_empty()).then_some(delta)
    }

    fn apply(&mut self, delta: Self::Delta) {
        for member in delta.removed {
            self.remove(&member);
        }
        for member in delta.added {
            self.insert(member);
        }
    }
}

impl<T: Serialize + DeserializeOwned> Diff for Arc<T>
where
    T: Clone + PartialEq,
{
    type Delta = T;

    fn diff(old: &Self, new: &Self) -> Option<T> {
        (!Arc::ptr_eq(old, new) && old != new).then(|| T::clone(new))
    }

    fn apply(&mut self, delta: T) {
        *self = Arc::new(delta);
    }
}

/// Defines a struct whose every field is `Diff`, and its delta: each field's own delta, sent
/// when that field changed.
macro_rules! diffed {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident, delta $delta:ident {
            $( $(#[$field_meta:meta])* $field_vis:vis $field:ident : $ty:ty ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        $vis struct $name {
            $( $(#[$field_meta])* $field_vis $field: $ty ),*
        }

        /// What changed in each field since the last one handed over (`diff::Diff`).
        #[derive(Default, serde::Deserialize, serde::Serialize)]
        $vis struct $delta {
            $( $field: Option<<$ty as $crate::diff::Diff>::Delta> ),*
        }

        impl $crate::diff::Diff for $name {
            type Delta = $delta;

            fn diff(old: &Self, new: &Self) -> Option<$delta> {
                let delta = $delta {
                    $( $field: $crate::diff::Diff::diff(&old.$field, &new.$field) ),*
                };
                let changed = false $( || delta.$field.is_some() )*;
                changed.then_some(delta)
            }

            fn apply(&mut self, delta: $delta) {
                $(
                    if let Some(change) = delta.$field {
                        $crate::diff::Diff::apply(&mut self.$field, change);
                    }
                )*
            }
        }
    };
}

pub(crate) use diffed;

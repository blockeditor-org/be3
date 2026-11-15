//! A single-threaded reactive graph: signals, memos, effects, and the scopes that own them.
//!
//! Handles (`Signal`, `Memo`, `Stored`, ...) are `Copy` keys into a thread-local arena, not
//! reference counts. Each belongs to the scope that was current when it was made and is
//! freed when that scope is disposed; handles and scopes cannot cross threads.
//!
//! A write never runs effects. It changes the value at once and queues the effects it
//! wakes, which run when the outermost `Scope::run`/`ScopeContext::run` ends, at
//! `flush()`, or when a zone with parked effects is entered (`enter_zone`).

extern crate self as beui_reactive;

mod arena;
mod async_memo;
mod computation;
mod keyed;
mod loading;
mod memo;
mod reader;
mod runtime;
mod scope;
mod selector;
mod signal;
mod source;
mod stored;
mod trackable;
mod trigger;

pub use async_memo::{AsyncMemo, LocalFuture, create_async_memo, spawn_with};
pub use beui_reactive_macros::Store;
pub use computation::{
    Effect, create_binding, create_effect, create_effect_on, create_effect_on_change,
    create_linked_signal,
};
pub use keyed::{KeyedItems, KeyedStore, Mapping, OwnedKeyedStore, numbered};
pub use loading::{Boundary, ErrorBoundary, Pending, Ready, hold};
pub use memo::{Compute, Fresh, FromPrevious, Memo, create_memo};
pub use reader::Reader;
pub use runtime::{
    ZoneGuard, check_tracked_read, component_body, enter_zone, flush, forget_zone, untrack,
    wake_with, wake_zone_with, zone_pending,
};
pub use scope::{
    Release, RootScope, Scope, ScopeContext, child_scope, current_scope, on_cleanup, on_release,
    owner_scope, provide_context, use_context,
};
pub use selector::{Selector, create_selector};
pub use signal::{Get, OwnedSignal, ReadSignal, Set, Signal, create_signal};
pub use source::{OwnedSource, Source, create_source};
pub use stored::{OwnedStored, Stored, StoredCell, store};
pub use trackable::Trackable;
pub use trigger::{OwnedTrigger, Trigger, create_trigger};

#[cfg(test)]
mod tests;

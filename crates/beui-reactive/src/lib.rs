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
mod computation;
mod keyed;
mod loading;
mod memo;
mod runtime;
mod scope;
mod selector;
mod signal;
mod stored;

pub use beui_reactive_macros::Store;
pub use computation::{Effect, create_binding, create_effect};
pub use keyed::{KeyedItems, KeyedStore, Mapping, OwnedKeyedStore, numbered};
pub use loading::{Boundary, Pending, Ready, hold, wait_for, wait_while};
pub use memo::{Memo, create_memo};
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
pub use stored::{OwnedStored, Stored, StoredCell, store};

#[cfg(test)]
mod tests;

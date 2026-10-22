extern crate self as reactive;

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

pub fn live_values() -> usize {
    arena::live()
}

pub use computation::{Effect, create_binding, create_effect};
pub use keyed::{KeyedItems, KeyedStore, Mapping, OwnedKeyedStore, numbered};
pub use loading::{Boundary, Pending, Ready, hold, wait_for, wait_while};
pub use memo::{Memo, create_memo};
pub use reactive_macros::Store;
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

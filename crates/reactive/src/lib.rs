extern crate self as reactive;

mod arena;
mod computation;
mod keyed;
mod memo;
mod runtime;
mod scope;
mod selector;
mod signal;
mod stored;

pub use computation::{Effect, create_effect};
pub use keyed::{KeyedItems, KeyedStore, Mapping, OwnedKeyedStore, numbered};
pub use memo::{Memo, create_memo};
pub use reactive_macros::Store;
pub use runtime::{ZoneGuard, batch, enter_zone, forget_zone, untrack, zone_pending};
pub use scope::{
    RootScope, Scope, ScopeContext, child_scope, current_scope, on_cleanup, owner_scope,
    provide_context, use_context,
};
pub use selector::{Selector, create_selector};
pub use signal::{OwnedSignal, ReadSignal, WriteSignal, create_signal};
pub use stored::{OwnedStored, Stored, StoredCell, store};

#[cfg(test)]
mod tests;

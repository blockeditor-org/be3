extern crate self as reactive;

mod computation;
mod keyed;
mod memo;
mod runtime;
mod scope;
mod selector;
mod signal;

pub use computation::{Effect, create_effect};
pub use keyed::{KeyedItems, KeyedStore};
pub use memo::{Memo, create_memo};
pub use reactive_macros::Store;
pub use runtime::{batch, settle, untrack};
pub use scope::{Scope, ScopeContext, on_cleanup, owner_scope, provide_context, use_context};
pub use selector::{Selector, create_selector};
pub use signal::{ReadSignal, WriteSignal, create_signal};

#[macro_export]
macro_rules! clone {
    ($($name:ident)* -> $body:expr) => {{
        $(let $name = $name.clone();)*
        $body
    }};
}

#[cfg(test)]
mod tests;

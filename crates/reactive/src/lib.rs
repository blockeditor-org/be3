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
pub use keyed::{KeyedItems, KeyedStore, Mapping, numbered};
pub use memo::{Memo, create_memo};
pub use reactive_macros::Store;
pub use runtime::{ZoneGuard, batch, enter_zone, forget_zone, untrack, zone_pending};
pub use scope::{
    RootScope, Scope, ScopeContext, current_scope, on_cleanup, owner_scope, provide_context,
    use_context,
};
pub use selector::{Selector, create_selector};
pub use signal::{OwnedSignal, ReadSignal, WriteSignal, create_signal};
pub use stored::{OwnedStored, Stored, store};

#[macro_export]
macro_rules! clone {
    ($($name:ident)* $(; $($raw:ident)*)? -> $body:expr) => {{
        $($(let $raw = $raw.clone();)*)?
        $(let $name = {
            #[allow(unused_imports)]
            use $crate::__clone::{PickClone as _, PickCopy as _};
            #[allow(non_camel_case_types, dead_code)]
            struct $name {}
            let handle = $crate::__clone::Handle::new($name.clone());
            $crate::__clone::Finish::<$name>::finish((&handle).pick())
        };)*
        $body
    }};
}

#[doc(hidden)]
pub mod __clone {
    pub struct Handle<T>(std::cell::Cell<Option<T>>);

    impl<T> Handle<T> {
        pub fn new(value: T) -> Self {
            Self(std::cell::Cell::new(Some(value)))
        }

        fn take(&self) -> T {
            self.0.take().expect("a clone! handle is picked once")
        }
    }

    pub struct Cloned<T>(T);

    pub struct AlreadyCopy<T>(T);

    pub trait PickCopy<T> {
        fn pick(&self) -> AlreadyCopy<T>;
    }

    impl<T: Copy> PickCopy<T> for Handle<T> {
        fn pick(&self) -> AlreadyCopy<T> {
            AlreadyCopy(self.take())
        }
    }

    pub trait PickClone<T> {
        fn pick(&self) -> Cloned<T>;
    }

    impl<T> PickClone<T> for &Handle<T> {
        fn pick(&self) -> Cloned<T> {
            Cloned(self.take())
        }
    }

    #[diagnostic::on_unimplemented(
        message = "`clone!` names `{Name}`, which is already `Copy`",
        label = "`{Name}` is `Copy`",
        note = "a `move` closure takes its own copy, so leave it out of the names before `->`"
    )]
    pub trait Finish<Name> {
        type Output;

        fn finish(self) -> Self::Output;
    }

    impl<T, Name> Finish<Name> for Cloned<T> {
        type Output = T;

        fn finish(self) -> T {
            self.0
        }
    }
}

#[cfg(test)]
mod tests;

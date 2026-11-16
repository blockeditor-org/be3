extern crate self as beui;

pub mod binding;
pub mod callback;
pub mod child_list;
pub mod children;
pub mod component;
pub mod control;
mod errored;
pub mod keep;
pub mod prop;
pub mod prop_value;

pub mod reactive {
    pub use crate::binding::Binding;
    pub use crate::callback::Callback;
    pub use crate::child_list::{ChildList, SlotId};
    pub use crate::children::{
        AtMostOneChild, Branch, ChildSegment, ChildSlot, Children, ChildrenSlot, ComponentOutput,
        Count, CountMany, CountNone, CountOne, CountZero, CountZeroOrOne, Counted, DynamicSegment,
        Fallback, FixedChild, FormOf, HostedNode, IntoChild, IntoSegment, IntoSegments,
        IntoSlotHost, LazyChild, NeedsFallback, OneChild, OneOf, Or, Run, SingleChild, SlotChild,
        SlotHost, Unwrapped, ValueFallback, ZeroOrOne, into_child, into_segment,
    };
    pub use crate::component::{
        Build, BuildFn, ChildValue, ComponentBuilder, ComponentContext, ComponentInfo,
        ComponentRecord, Render, RenderFn, component, current_component, try_current_component,
    };
    pub use crate::control::{
        Case, CaseIs, CaseOf, CaseWhen, Dynamic, ForEach, ForEachFill, ForEachRef, Keyed, Loading,
        Match, Matched, RowArgs, RowKey, Show, ShowValue, ShownRow,
    };
    pub use crate::errored::Errored;
    pub use crate::into_prop_via;
    pub use crate::one_child_slot;
    pub use crate::keep::{AnyKey, KeepSet, KeptKey};
    pub use crate::keep::{kept_shown, provide_kept_shown};
    pub use crate::prop::{Func, IntoLiftedProp, IntoProp, IntoPropFrom, IntoReadProp, Prop};
    pub use crate::prop_value::{
        AbsentProp, IntoPropValue, MarkedRequired, Takes, TakesBlock, TakesOptionalBlock,
    };
    pub use beui_macros::{component, view};
    pub use beui_reactive::{
        AsyncMemo, Boundary, Effect, ErrorBoundary, Get, KeyedItems, KeyedStore, Memo,
        OwnedKeyedStore, OwnedSignal, OwnedSource, OwnedStored, OwnedTrigger, Pending, ReadSignal,
        Reader, Ready, RootScope, Scope, ScopeContext, Selector, Set, Signal, Source, Store,
        Stored, StoredCell, Trackable, Trigger, child_scope, create_async_memo, create_binding,
        create_effect, create_effect_on, create_effect_on_change, create_linked_signal,
        create_memo, create_selector, create_signal, create_source, create_trigger, flush, hold,
        on_cleanup, owner_scope, provide_context, spawn_with, store, untrack, use_context,
        zone_pending,
    };

    /// What the `#[component]` and `view!` macros name by absolute path: a module, so that
    /// `reactive::*` does not bring these names in, and not hidden, so that messages name them
    /// short.
    pub mod macro_support {
        pub use crate::component::MissingProp;
        pub use crate::prop::{ByClosure, ByClosureLifted, ByLift, ByValue, Given};
        pub use crate::prop_value::{
            ByExact, ByInto, ByProp,
            BySignal, BySome, ByTakes, ByUnitClosure, ChildBlock, ChildClosure, ChildShape,
            ChildUnitClosure, ChildWith, Exact, ExactTakes, ExactWithin, ExactWithinTakes, IntoExactProp,
            FnOnceSlot, FnSlot, Is, Itself, NoHandedSlot, OptionalPropTakes, OptionalTakes, PropOr, PropTakes, ReadProp, ReadPropTakes,
            HandsAValue, SameType, SetProp, Slot, TakesChildren, TakesClosure, TakesNoChildren, TakesUnitClosure, TakesWith,
        };
    }
}

#[cfg(test)]
mod tests;

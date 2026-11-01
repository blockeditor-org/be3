extern crate self as beui;

pub mod binding;
pub mod callback;
pub mod child_list;
pub mod children;
pub mod component;
pub mod control;
pub mod prop;

pub mod reactive {
    pub use crate::binding::{Binding, IntoBinding};
    pub use crate::callback::{Callback, ClickCallback, IntoCallback, IntoClickCallback};
    pub use crate::child_list::{ChildList, SlotId};
    pub use crate::children::{
        AtMostOneChild, ChildSegment, ChildSlot, Children, ChildrenSlot, DynamicSegment,
        FixedChild, IntoChild, IntoSegment, IntoSegments, IntoSlotHost, OneChild, Run, SingleChild,
        SlotChild, SlotHost, into_child, into_segment,
    };
    pub use crate::component::{
        BuildsItsChildren, ChildValue, ComponentBuilder, ComponentContext, ComponentInfo,
        ComponentRecord, HandsAValue, IntoRender, IntoRenderFn, MissingProp, Render, RenderFn,
        component, current_component, try_current_component,
    };
    pub use crate::control::{Dynamic, ForEach, Keyed, Loading, Show, ShowKeepAlive};
    pub use crate::into_prop_via;
    pub use crate::prop::{Func, Getter, IntoFunc, IntoProp, Prop};
    pub use beui_macros::{component, view};
    pub use reactive::{
        Boundary, Effect, Get, KeyedItems, KeyedStore, Memo, OwnedKeyedStore, OwnedSignal,
        OwnedStored, Pending, ReadSignal, Ready, RootScope, Scope, ScopeContext, Selector, Set,
        Signal, Store, Stored, StoredCell, child_scope, create_binding, create_effect, create_memo,
        create_selector, create_signal, flush, hold, on_cleanup, owner_scope, provide_context,
        store, untrack, use_context, wait_for, wait_while, zone_pending,
    };
}

#[cfg(test)]
mod tests;

extern crate self as beui;

pub mod callback;
pub mod child_list;
pub mod children;
pub mod component;
pub mod control;
pub mod prop;

pub mod reactive {
    pub use crate::callback::{Callback, ClickCallback};
    pub use crate::child_list::{ChildList, SlotId};
    pub use crate::children::{
        AtMostOneChild, ChildSegment, ChildSlot, Children, ChildrenSlot, DynamicSegment,
        FixedChild, IntoChild, IntoSegment, IntoSegments, IntoSlotHost, OneChild, Run, SingleChild,
        SlotChild, SlotHost, into_child, into_segment,
    };
    pub use crate::component::{
        ChildScope, ChildValue, ComponentBuilder, ComponentContext, IntoRender, IntoRenderFn,
        MissingProp, Render, RenderFn, UnitHandle, component, current_component,
        try_current_component,
    };
    pub use crate::control::{Dynamic, ForEach, Keyed, Show, ShowKeepAlive};
    pub use crate::prop::{Func, IntoFunc, IntoProp, Prop};
    pub use beui_macros::{component, view};
    pub use reactive::{
        Effect, KeyedItems, KeyedStore, Memo, ReadSignal, Scope, ScopeContext, Selector, Store,
        WriteSignal, batch, clone, create_effect, create_memo, create_selector, create_signal,
        on_cleanup, owner_scope, provide_context, settle, untrack, use_context, zone_pending,
    };
}

#[cfg(test)]
mod tests;

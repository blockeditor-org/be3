use beui_components_unstyled as unstyled;
use beui_components_unstyled::GlideApi;
use beui_macros::{component, view};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Pos2;

use crate::tokens::Movement;

/// `unstyled::Glide` settling on the theme's `Movement::Settle`: animates its child
/// from where it was to where layout puts it. A change to `on` makes the next move
/// glide rather than jump; `from` starts one from a window point.
#[component]
pub fn Glide(
    #[prop(default = ComponentRef::new())] api: ComponentRef<GlideApi>,
    on: Option<Trackable>,
    #[prop(default = None)] from: Prop<Option<Pos2>>,
    #[prop(default = false)] raised: Prop<bool>,
    within: Option<ComponentRef<GlideApi>>,
    children: NodeId,
) -> NodeId {
    view! {
        <unstyled::Glide api spring={Movement::Settle.spring()} on from raised within>
            {children}
        </unstyled::Glide>
    }
}

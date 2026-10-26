pub use crate::core::overlay::{OverlayPresence, use_overlay_presence};
use crate::reactive::Callback;
use crate::reactive::NodeRef;
use crate::reactive::Prop;
use crate::reactive::RenderFn;
use beui_core::base::overlay::OverlayAnchor;
use beui_core::base::overlay::Placement;
use beui_core::color::Color32;
use beui_core::layer::Dismiss;
use beui_core::layer::LayerLevel;
use beui_core::layer::LayerPolicy;
use beui_core::node::NodeId;
use beui_core::window::WindowId;
use beui_macros::component;

#[component]
pub fn Overlay(
    anchor: Prop<OverlayAnchor>,
    #[prop(default = Placement::BelowStart)] placement: Prop<Placement>,
    #[prop(default = Color32::TRANSPARENT)] scrim: Prop<Color32>,
    #[prop(default = LayerPolicy::BlocksAll)] policy: Prop<LayerPolicy>,
    #[prop(default = LayerLevel::Normal)] level: Prop<LayerLevel>,
    #[prop(default = true)] traps_focus: Prop<bool>,
    #[prop(default = None)] shown_in: Prop<Option<WindowId>>,
    trigger: Option<NodeRef>,
    pointer: Option<NodeRef>,
    open: Prop<bool>,
    #[prop(default = false)] keep_alive: bool,
    on_dismiss: Callback<Dismiss>,
    #[prop(children)] content: RenderFn<()>,
) -> NodeId {
    crate::core::overlay(
        anchor,
        placement,
        scrim,
        policy,
        level,
        traps_focus,
        shown_in,
        trigger,
        pointer,
        open,
        keep_alive,
        on_dismiss,
        content,
    )
}

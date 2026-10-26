pub use crate::core::overlay::{OverlayPresence, use_overlay_presence};
use crate::reactive::Callback;
use crate::reactive::NodeId;
use crate::reactive::NodeRef;
use crate::reactive::OverlayAnchor;
use crate::reactive::Prop;
use crate::reactive::RenderFn;
use crate::types::Color32;
use crate::types::Dismiss;
use crate::types::LayerLevel;
use crate::types::LayerPolicy;
use crate::types::Placement;
use crate::types::WindowId;
use crate::types::WindowSpec;
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
    #[prop(default = 0.0)] beak: Prop<f32>,
    #[prop(default = None)] window: Prop<Option<WindowSpec>>,
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
        beak,
        window,
        open,
        keep_alive,
        on_dismiss,
        content,
    )
}

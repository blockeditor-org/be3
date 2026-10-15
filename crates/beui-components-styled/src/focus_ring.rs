use beui_macros::{component, view};

use crate::theme::{FOCUS_RING_WIDTH, RADIUS, use_theme};
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

#[component]
pub fn FocusRing(
    focused: Prop<bool>,
    #[prop(default = RADIUS)] radius: f32,
    #[prop(default = 0.0)] offset: f32,
    children: Child,
) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame
            outline={focus_outline(focused, theme.accent, FOCUS_RING_WIDTH, offset)}
            radius=radius
        >
            {children}
        </Frame>
    }
}

use beui_macros::{component, view};

use crate::theme::{FOCUS_RING_WIDTH, RADIUS, use_theme};
use beui_core::node::NodeId;
use beui_view::reactive::{Child, Frame, Prop, focus_ring};

#[component]
pub fn FocusRing(
    focused: Prop<bool>,
    #[prop(default = RADIUS)] radius: u8,
    #[prop(default = 0.0)] offset: f32,
    children: Child,
) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame
            outline={theme.accent.clone()}
            outline_width=FOCUS_RING_WIDTH
            radius
            outline_offset=offset
            outline_visible={focus_ring(focused)}
        >
            {children}
        </Frame>
    }
}

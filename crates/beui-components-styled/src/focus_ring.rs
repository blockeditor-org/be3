use beui_macros::{component, view};

use crate::frame::{Border, Frame, Radius, focus_border};
use beui_core::node::NodeId;
use beui_view::reactive::*;

#[component]
pub fn FocusRing(
    focused: Prop<bool>,
    #[prop(default = Radius::Md)] radius: Radius,
    #[prop(default = 0.0)] offset: f32,
    children: Child,
) -> NodeId {
    view! {
        <Frame border={focus_border(focused, Border::FOCUS.offset(offset))} radius>
            {children}
        </Frame>
    }
}

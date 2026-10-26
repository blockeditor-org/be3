use beui_macros::{component, view};

use crate::frame::{Border, Frame, Radius, focus_border};
use beui_core::node::NodeId;
use beui_view::reactive::*;

#[component]
pub fn FocusRing(
    focused: Prop<bool>,
    #[prop(default = Radius::Md)] radius: Radius,
    #[prop(default = 0.0)] offset: Prop<f32>,
    children: Child,
) -> NodeId {
    let ring = focus_border(focused, Border::FOCUS);
    let border = create_memo(move || ring.get().map(|border| border.offset(offset.get())));
    view! {
        <Frame border={border} radius>{children}</Frame>
    }
}

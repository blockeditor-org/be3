use beui_macros::{component, view};

use crate::node::NodeId;
use crate::reactive::{Child, Frame};
use crate::styled::theme::{BORDER, BORDER_WIDTH};

#[component]
pub fn Bordered(corner_radius: u8, children: Child) -> NodeId {
    view! {
        <Frame outline=BORDER outline_width=BORDER_WIDTH radius={corner_radius} outline_offset=0.0 outline_visible=true>
            {children}
        </Frame>
    }
}

#[component]
pub fn Separator() -> NodeId {
    view! { <Frame color=BORDER radius=0></Frame> }
}

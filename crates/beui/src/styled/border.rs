use beui_macros::{component, view};

use crate::node::NodeId;
use crate::reactive::{Child, Frame};
use crate::styled::theme::{BORDER_WIDTH, use_theme};

#[component]
pub fn Bordered(corner_radius: u8, children: Child) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame
            outline={theme.border.clone()}
            outline_width=BORDER_WIDTH
            radius=corner_radius
            outline_offset=0.0
            outline_visible=true
        >
            {children}
        </Frame>
    }
}

#[component]
pub fn Separator() -> NodeId {
    let theme = use_theme();
    view! {
        <Frame color={theme.border.clone()} radius=0></Frame>
    }
}

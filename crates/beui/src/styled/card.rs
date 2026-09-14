use beui_macros::{component, view};

use crate::node::NodeId;
use crate::reactive::{Child, Frame};
use crate::styled::theme::{BORDER, BORDER_WIDTH, CARD_RADIUS, SURFACE};

const PADDING_HORIZONTAL: f32 = 18.0;
const PADDING_VERTICAL: f32 = 16.0;

#[component]
pub fn Card(children: Child) -> NodeId {
    view! {
        <Frame color=SURFACE outline=BORDER outline_width=BORDER_WIDTH radius=CARD_RADIUS outline_visible=true padding_horizontal=PADDING_HORIZONTAL padding_vertical=PADDING_VERTICAL>
            {children}
        </Frame>
    }
}

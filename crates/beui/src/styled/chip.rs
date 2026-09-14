use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::node::NodeId;
use crate::reactive::{create_memo, Frame, Prop, Text};
use crate::styled::theme::{BORDER, BORDER_WIDTH, CHIP_RADIUS, FONT_SMALL, SURFACE_RAISED, TEXT};

const PADDING_HORIZONTAL: f32 = 8.0;
const PADDING_VERTICAL: f32 = 3.0;

#[component]
pub fn Chip(label: Prop<String>) -> NodeId {
    let label_text = create_memo(move || label.get());
    view! {
        <Frame color=SURFACE_RAISED outline=BORDER outline_width=BORDER_WIDTH radius=CHIP_RADIUS outline_visible=true padding_horizontal=PADDING_HORIZONTAL padding_vertical=PADDING_VERTICAL>
            <Text string={label_text} font_size=FONT_SMALL color=TEXT align=TextAlign::Center />
        </Frame>
    }
}

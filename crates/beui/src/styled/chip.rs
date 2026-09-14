use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::node::NodeId;
use crate::reactive::{Frame, Prop, Text, create_memo};
use crate::styled::theme::{BORDER_WIDTH, CHIP_RADIUS, FONT_SMALL, use_theme};

const PADDING_HORIZONTAL: f32 = 8.0;
const PADDING_VERTICAL: f32 = 3.0;

#[component]
pub fn Chip(label: Prop<String>) -> NodeId {
    let label_text = create_memo(move || label.get());
    let theme = use_theme();
    view! {
        <Frame
            color={theme.surface_raised.clone()}
            outline={theme.border.clone()}
            outline_width=BORDER_WIDTH
            radius=CHIP_RADIUS
            outline_visible=true
            padding_horizontal=PADDING_HORIZONTAL
            padding_vertical=PADDING_VERTICAL
        >
            <Text
                string={label_text}
                font_size=FONT_SMALL
                color={theme.text.clone()}
                align=TextAlign::Center
            />
        </Frame>
    }
}

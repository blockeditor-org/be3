use beui_macros::{component, view};

use crate::node::NodeId;
use crate::reactive::{Child, Frame};
use crate::styled::theme::{BORDER_WIDTH, CARD_RADIUS, use_theme};

const PADDING_HORIZONTAL: f32 = 18.0;
const PADDING_VERTICAL: f32 = 16.0;

#[component]
pub fn Card(children: Child) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame
            color={theme.pick(|theme| theme.surface)}
            outline={theme.pick(|theme| theme.border)}
            outline_width=BORDER_WIDTH
            radius=CARD_RADIUS
            outline_visible=true
            padding_horizontal=PADDING_HORIZONTAL
            padding_vertical=PADDING_VERTICAL
        >
            {children}
        </Frame>
    }
}

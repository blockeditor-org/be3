use beui_macros::{component, view};

use crate::text::{Text, TextSize};
use crate::theme::{BORDER_WIDTH, CHIP_RADIUS, use_theme};
use beui_core::base::TextAlign;
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const PADDING_HORIZONTAL: f32 = 8.0;
const PADDING_VERTICAL: f32 = 3.0;

#[component]
pub fn Chip(label: Prop<String>) -> NodeId {
    let label_text = create_memo(move || label.get());
    let theme = use_theme();
    view! {
        <Frame
            fill={theme.surface_raised}
            outline={create_memo(move || Some(Outline::new(theme.border.get(), BORDER_WIDTH)))}
            radius=CHIP_RADIUS
            padding={Sides::symmetric(PADDING_HORIZONTAL, PADDING_VERTICAL)}
        >
            <Text string={label_text} size=TextSize::Sm align=TextAlign::Center />
        </Frame>
    }
}

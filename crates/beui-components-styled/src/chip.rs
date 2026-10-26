use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_core::font::Overflow;
use beui_macros::{component, view};

use crate::text::{Text, TextSize};
use beui_core::base::TextAlign;
use beui_core::node::NodeId;
use beui_view::reactive::*;

const PADDING_HORIZONTAL: f32 = 8.0;
const PADDING_VERTICAL: f32 = 3.0;

#[component]
pub fn Chip(label: Prop<String>) -> NodeId {
    let label_text = create_memo(move || label.get());
    view! {
        <Frame
            fill=ColorRole::SurfaceRaised
            border={Border::DEFAULT}
            radius=Radius::Sm
            padding={Sides::symmetric(PADDING_HORIZONTAL, PADDING_VERTICAL)}
        >
            <Text overflow=Overflow::Ellipsis size=TextSize::Sm align=TextAlign::Center>
                {label_text}
            </Text>
        </Frame>
    }
}

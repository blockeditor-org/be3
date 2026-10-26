use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_core::font::Overflow;
use beui_macros::{component, view};

use crate::text::{Text, TextSize};
use beui_core::base::TextAlign;
use beui_core::node::NodeId;
use beui_view::reactive::*;

use crate::tokens::{Padding, Space};

const PADDING: Padding = Padding::symmetric(Space::Md, Space::Xs2);

#[component]
pub fn Chip(label: Prop<String>) -> NodeId {
    let label_text = create_memo(move || label.get());
    view! {
        <Frame
            fill=ColorRole::SurfaceRaised
            border={Border::DEFAULT}
            radius=Radius::Sm
            padding=PADDING
        >
            <Text overflow=Overflow::Ellipsis size=TextSize::Sm align=TextAlign::Center>
                {label_text}
            </Text>
        </Frame>
    }
}

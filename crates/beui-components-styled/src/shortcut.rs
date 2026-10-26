use beui_macros::{component, view};

use crate::Chip;
use crate::text::{Text, TextColor, TextSize};
use beui_core::base::TextAlign;
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const SPACING: f32 = 10.0;

#[component]
pub fn Shortcut(keys: Prop<String>, description: Prop<String>) -> NodeId {
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
            <Chip label={keys} />
            <Text
                @sizing=ItemSize::Percent(100.0)
                size=TextSize::Sm
                color=TextColor::Muted
                align=TextAlign::Start
            >
                {description}
            </Text>
        </List>
    }
}

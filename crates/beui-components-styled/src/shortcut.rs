use beui_macros::{component, view};

use crate::Chip;
use crate::list::List;
use crate::text::{Text, TextColor, TextSize};
use crate::tokens::Space;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::TextAlign;

const SPACING: Space = Space::Md;

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

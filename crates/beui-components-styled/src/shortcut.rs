use beui_macros::{component, view};

use crate::Chip;
use crate::theme::{FONT_SMALL, use_theme};
use beui_core::base::TextAlign;
use beui_core::node::NodeId;
use beui_view::reactive::{Align, Direction, ItemSize, List, Prop, Text};

const SPACING: f32 = 10.0;

#[component]
pub fn Shortcut(keys: Prop<String>, description: Prop<String>) -> NodeId {
    let theme = use_theme();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
            <Chip label={keys} />
            <Text
                @sizing=ItemSize::Percent(100.0)
                string={description}
                font_size=FONT_SMALL
                color={theme.text_muted.clone()}
                align=TextAlign::Start
                wrap=true
            />
        </List>
    }
}

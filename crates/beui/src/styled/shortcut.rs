use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::node::NodeId;
use crate::reactive::{CenteredRow, ItemSize, Prop, Text};
use crate::styled::Chip;
use crate::styled::theme::{FONT_SMALL, use_theme};

const SPACING: f32 = 10.0;

#[component]
pub fn Shortcut(keys: Prop<String>, description: Prop<String>) -> NodeId {
    let theme = use_theme();
    view! {
        <CenteredRow spacing=SPACING>
            <Chip label={keys} />
            <Text
                @sizing=ItemSize::Percent(100.0)
                string={description}
                font_size=FONT_SMALL
                color={theme.text_muted.clone()}
                align=TextAlign::Start
                wrap=true
            />
        </CenteredRow>
    }
}

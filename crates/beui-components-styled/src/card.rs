use beui_macros::{component, view};

use crate::theme::{BORDER_WIDTH, CARD_RADIUS, use_theme};
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const PADDING_HORIZONTAL: f32 = 18.0;
const PADDING_VERTICAL: f32 = 16.0;

#[component]
pub fn Card(children: Child) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame
            fill={theme.surface}
            outline={create_memo(move || Some(Outline::new(theme.border.get(), BORDER_WIDTH)))}
            radius=CARD_RADIUS
            padding={Sides::symmetric(PADDING_HORIZONTAL, PADDING_VERTICAL)}
        >
            {children}
        </Frame>
    }
}

use beui_macros::{component, view};

use crate::node::NodeId;
use crate::reactive::{Child, Frame, Prop};
use std::time::Duration;
use crate::styled::text::Caption;
use crate::styled::theme::{BORDER_WIDTH, RADIUS, use_theme};
use crate::unstyled;
use crate::unstyled::{TOOLTIP_DELAY, TooltipHandle};

const PADDING_HORIZONTAL: f32 = 8.0;
const PADDING_VERTICAL: f32 = 4.0;
const GAP: f32 = 4.0;
const MAX_WIDTH: f32 = 280.0;

#[component]
pub fn Tooltip(
    label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = TOOLTIP_DELAY)] delay: Duration,
    children: Child,
) -> NodeId {
    view! {
        <unstyled::Tooltip
            label
            disabled
            delay
            content={move |handle: TooltipHandle| view! { <TooltipBubble handle /> }}
        >
            {children}
        </unstyled::Tooltip>
    }
}

#[component]
fn TooltipBubble(handle: TooltipHandle) -> NodeId {
    let TooltipHandle { label, .. } = handle;
    let theme = use_theme();
    view! {
        <Frame padding_vertical=GAP max_width=MAX_WIDTH>
            <Frame
                color={theme.surface_raised.clone()}
                outline={theme.border.clone()}
                outline_width=BORDER_WIDTH
                outline_visible=true
                radius=RADIUS
                padding_horizontal=PADDING_HORIZONTAL
                padding_vertical=PADDING_VERTICAL
            >
                <Caption content={label} color={theme.text.clone()} />
            </Frame>
        </Frame>
    }
}

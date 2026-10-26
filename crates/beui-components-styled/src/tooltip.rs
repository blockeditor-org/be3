use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_macros::{component, view};

use crate::text::{Text, TextColor, TextStyle};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{TOOLTIP_DELAY, TooltipHandle};
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;
use std::time::Duration;

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
            content={move |handle: TooltipHandle| view! {
                <TooltipBubble handle />
            }}
        >
            {children}
        </unstyled::Tooltip>
    }
}

#[component]
fn TooltipBubble(handle: TooltipHandle) -> NodeId {
    let TooltipHandle { label, .. } = handle;
    view! {
        <Frame padding={Sides::vertical(GAP)}>
            <List direction=Direction::Horizontal spacing=0.0>
                <Frame
                    @sizing={ItemSize::Intrinsic.max(MAX_WIDTH)}
                    @test_id={"tooltip"}
                    fill=ColorRole::SurfaceRaised
                    border={Border::DEFAULT}
                    radius=Radius::Md
                    padding={Sides::symmetric(PADDING_HORIZONTAL, PADDING_VERTICAL)}
                >
                    <Text style=TextStyle::CAPTION color=TextColor::Default>{label}</Text>
                </Frame>
            </List>
        </Frame>
    }
}

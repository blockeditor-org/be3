use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_macros::{component, view};

use crate::list::List;
use crate::text::{Text, TextColor, TextStyle};
use crate::tokens::{Padding, Space};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{TOOLTIP_DELAY, TooltipHandle};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::{Direction, ItemSize};
use std::time::Duration;

const PADDING: Padding = Padding::symmetric(Space::Md, Space::Xs);
const GAP: Space = Space::Xs;
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
        <Frame padding={Padding::vertical(GAP)}>
            <List direction=Direction::Horizontal spacing=Space::None>
                <Frame
                    @sizing={ItemSize::Intrinsic.max(MAX_WIDTH)}
                    @test_id={"tooltip"}
                    fill=ColorRole::SurfaceRaised
                    border={Border::DEFAULT}
                    radius=Radius::Md
                    padding=PADDING
                >
                    <Text style=TextStyle::CAPTION color=TextColor::Default>{label}</Text>
                </Frame>
            </List>
        </Frame>
    }
}

use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_macros::{component, view};

use crate::list::List;
use crate::text::{Text, TextColor, TextStyle};
use crate::tokens::{Padding, Space};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{TOOLTIP_DELAY, TooltipHandle};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Direction;
use beui_view::types::{Placement, Pos2, Rect};
use std::time::Duration;

const PADDING: Padding = Padding::symmetric(Space::Md, Space::Xs);
const MAX_WIDTH: f32 = 280.0;

/// Where a tooltip opens unless told otherwise: below the start of what it describes,
/// `Space::Xs` off it.
pub const TOOLTIP_PLACEMENT: Placement = Placement::BELOW_START.gap(Space::Xs.px() as u16);

#[component]
pub fn Tooltip(
    label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = TOOLTIP_DELAY)] delay: Duration,
    #[prop(default = false)] open: Binding<bool>,
    #[prop(default = None)] at: Binding<Option<Pos2>>,
    #[prop(default = TOOLTIP_PLACEMENT)] placement: Prop<Placement>,
    #[prop(default = None)] bounds: Prop<Option<Rect>>,
    #[prop(default = false)] keep_alive: bool,
    children: Child,
) -> NodeId {
    view! {
        <unstyled::Tooltip
            label
            disabled
            delay
            open
            at
            placement
            bounds
            keep_alive
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
        <List direction=Direction::Horizontal spacing=Space::None>
            <ListItem max=MAX_WIDTH>
                <Frame
                    @test_id={"tooltip"}
                    fill=ColorRole::Surface
                    border={Border::DEFAULT}
                    radius=Radius::Md
                    padding=PADDING
                >
                    <Text style=TextStyle::CAPTION color=TextColor::Default>{label}</Text>
                </Frame>
            </ListItem>
        </List>
    }
}

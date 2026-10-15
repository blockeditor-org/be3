use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_macros::{component, view};

use crate::Scrollbar;
use crate::text::{Icon, Text};
use crate::theme::{SCROLL_FADE, SCROLLBAR_SPACING, SCROLLBAR_WIDTH, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{ScrollHandle, ScrollbarStyle};
use beui_core::base::{Align, Direction, ItemSize, ScrollPosition};
use beui_core::font::TextAlign;
use beui_core::icons::{ICON_HEIGHT, ICON_WIDTH};
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const MARKER_SIZE: f32 = 28.0;
const MARKER_RADIUS: f32 = 14.0;

#[component]
pub fn Scroll(
    #[prop(default = 0.0)] offset: Prop<f32>,
    #[prop(default = None)] reveal: Prop<Option<usize>>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = None)] max_length: Option<f32>,
    on_change: Callback<ScrollPosition>,
    children: Children<ListChild>,
) -> NodeId {
    let theme = use_theme();
    let along = direction;
    let scroll = view! {
        <unstyled::Scroll
            offset
            reveal
            direction
            fit={max_length.is_some()}
            focus_color={theme.accent}
            scrollbar={scrollbar_style()}
            marker={|axis: Memo<Direction>| view! {
                <AutoscrollMarker axis />
            }}
            on_change={on_change}
        >
            {children}
        </unstyled::Scroll>
    };
    let Some(max_length) = max_length else {
        return scroll;
    };
    view! {
        <List direction={along} spacing=0.0>
            {scroll} @sizing={ItemSize::Intrinsic.max(max_length)}
        </List>
    }
}

pub fn scrollbar_style() -> ScrollbarStyle {
    ScrollbarStyle::new(SCROLLBAR_SPACING, |handle: ScrollHandle| {
        let ScrollHandle {
            position,
            direction,
            scroll_to,
        } = handle;
        view! {
            <Scrollbar
                @sizing=ItemSize::Fixed(SCROLLBAR_WIDTH)
                position
                direction
                on_scroll_to={move |offset: f32| scroll_to.call(offset)}
            />
        }
    })
    .fading(SCROLL_FADE)
}

#[component]
fn AutoscrollMarker(axis: Prop<Direction>) -> NodeId {
    let glyph = axis.map(|axis| {
        match axis {
            Direction::Vertical => ICON_HEIGHT,
            Direction::Horizontal => ICON_WIDTH,
        }
        .to_string()
    });
    view! {
        <Frame
            width=MARKER_SIZE
            height=MARKER_SIZE
            radius={Radius::custom(MARKER_RADIUS)}
            fill=ColorRole::SurfaceRaised
            border={Border::DEFAULT}
        >
            <List align=Align::Center spacing=0.0>
                <Spacer @sizing=ItemSize::Percent(50.0) />
                <Text align=TextAlign::Center>
                    <Icon glyph={glyph} />
                </Text>
                <Spacer @sizing=ItemSize::Percent(50.0) />
            </List>
        </Frame>
    }
}

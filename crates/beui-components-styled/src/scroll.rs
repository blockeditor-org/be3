use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_macros::{component, view};

use crate::Scrollbar;
use crate::card;
use crate::dialog;
use crate::popover::PANEL_PADDING;
use crate::text::{Icon, Text};
use crate::theme::{BORDER_WIDTH, SCROLL_FADE, SCROLLBAR_WIDTH, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{ScrollHandle, ScrollbarStyle};
use beui_core::base::{Align, Direction, ItemSize, ScrollPosition, Sides};
use beui_core::font::TextAlign;
use beui_core::icons::{ICON_MOVE_HORIZONTAL, ICON_MOVE_VERTICAL};
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = -1.0;
const MARKER_SIZE: f32 = 28.0;
const MARKER_RADIUS: f32 = 14.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Bleed {
    reach: Sides,
}

impl Bleed {
    pub const NONE: Bleed = Bleed::new(0.0, 0.0);
    pub const POPOVER: Bleed = Bleed::new(PANEL_PADDING, PANEL_PADDING);
    pub const CARD: Bleed = Bleed::new(card::PADDING_HORIZONTAL, card::PADDING_VERTICAL);
    pub const DIALOG: Bleed = Bleed::new(dialog::PADDING_HORIZONTAL, dialog::PADDING_VERTICAL);

    pub(crate) const fn dialog(below_title: bool, above_actions: bool) -> Self {
        let reach = Bleed::DIALOG.reach;
        let reach = if below_title {
            reach.top(dialog::SPACING)
        } else {
            reach
        };
        let reach = if above_actions {
            reach.bottom(dialog::SPACING)
        } else {
            reach
        };
        Self { reach }
    }

    const fn new(horizontal: f32, vertical: f32) -> Self {
        Self {
            reach: Sides::symmetric(inside_border(horizontal), inside_border(vertical)),
        }
    }

    pub const fn left(self, on: bool) -> Self {
        Self {
            reach: self.reach.left(if on { self.reach.left } else { 0.0 }),
        }
    }

    pub const fn top(self, on: bool) -> Self {
        Self {
            reach: self.reach.top(if on { self.reach.top } else { 0.0 }),
        }
    }

    pub const fn right(self, on: bool) -> Self {
        Self {
            reach: self.reach.right(if on { self.reach.right } else { 0.0 }),
        }
    }

    pub const fn bottom(self, on: bool) -> Self {
        Self {
            reach: self.reach.bottom(if on { self.reach.bottom } else { 0.0 }),
        }
    }
}

const fn inside_border(padding: f32) -> f32 {
    if padding > BORDER_WIDTH {
        padding - BORDER_WIDTH
    } else {
        0.0
    }
}

#[component]
pub fn Scroll(
    #[prop(default = ComponentRef::new())] api: ComponentRef<unstyled::ScrollApi>,
    #[prop(default = 0.0)] offset: Prop<f32>,
    #[prop(default = None)] reveal: Prop<Option<usize>>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = false)] fit: bool,
    #[prop(default = None)] max_length: Option<f32>,
    #[prop(default = false)] follow_end: Prop<bool>,
    #[prop(default = Bleed::NONE)] bleed: Prop<Bleed>,
    on_change: Callback<ScrollPosition>,
    children: Children<ListChild>,
) -> NodeId {
    let theme = use_theme();
    let scroll = view! {
        <unstyled::Scroll
            api
            offset
            reveal
            direction
            fit={fit || max_length.is_some()}
            follow_end
            bleed={bleed.map(|bleed| bleed.reach)}
            focus_ring={create_memo(move || Some(Outline::new(theme.accent.get(), FOCUS_RING_WIDTH).offset(FOCUS_RING_OFFSET)))}
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
        <List direction={direction} spacing=0.0>
            {scroll} @sizing={ItemSize::Intrinsic.max(max_length)}
        </List>
    }
}

pub fn scrollbar_style() -> ScrollbarStyle {
    ScrollbarStyle::new(SCROLLBAR_WIDTH, |handle: ScrollHandle| {
        let ScrollHandle {
            position,
            direction,
            scroll_to,
            visibility,
            on_engage,
        } = handle;
        view! {
            <Scrollbar
                position
                direction
                visibility
                on_scroll_to={move |offset: f32| scroll_to.call(offset)}
                on_engage={move |engaged: bool| on_engage.call(engaged)}
            />
        }
    })
    .fading(SCROLL_FADE)
}

#[component]
fn AutoscrollMarker(axis: Prop<Direction>) -> NodeId {
    let glyph = axis.map(|axis| match axis {
        Direction::Vertical => ICON_MOVE_VERTICAL,
        Direction::Horizontal => ICON_MOVE_HORIZONTAL,
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

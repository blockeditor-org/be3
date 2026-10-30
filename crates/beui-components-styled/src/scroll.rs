use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_macros::{component, view};

use crate::Scrollbar;
use crate::card;
use crate::dialog;
use crate::list::List;
use crate::popover::PANEL_PADDING;
use crate::text::{Icon, Text};
use crate::theme::{SCROLL_FADE, SCROLLBAR_WIDTH, use_theme};
use crate::tokens::{Padding, Size, Space, Stroke, use_size};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{ScrollHandle, ScrollbarStyle};
use beui_view::primitives::Spacer;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Outline;
use beui_view::types::TextAlign;
use beui_view::types::icons::{ICON_MOVE_HORIZONTAL, ICON_MOVE_VERTICAL};
use beui_view::types::{Align, Direction, ItemSize, ScrollPosition, Sides};

const FOCUS_RING_WIDTH: Stroke = Stroke::Focus;
const FOCUS_RING_OFFSET: Stroke = Stroke::Custom(-1.0);
const MARKER_SIZE: Size = Size::Custom(28.0);

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Bleed {
    reach: Sides,
}

impl Bleed {
    pub const NONE: Bleed = Bleed::new(0.0, 0.0);
    pub const POPOVER: Bleed = Bleed::padded(Padding::all(PANEL_PADDING));
    pub const CARD: Bleed = Bleed::padded(Padding::all(card::PADDING));
    pub const DIALOG: Bleed = Bleed::padded(dialog::PADDING);

    pub(crate) const fn dialog(below_title: bool, above_actions: bool) -> Self {
        let reach = Bleed::DIALOG.reach;
        let reach = if below_title {
            reach.top(dialog::SPACING.px())
        } else {
            reach
        };
        let reach = if above_actions {
            reach.bottom(dialog::SPACING.px())
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

    const fn padded(padding: Padding) -> Self {
        let sides = padding.px();
        Self::new(sides.left, sides.top)
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
    if padding > Stroke::Hairline.px() {
        padding - Stroke::Hairline.px()
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
    #[prop(default = None)] max_length: Prop<Option<Size>>,
    #[prop(default = false)] follow_end: Prop<bool>,
    #[prop(default = Bleed::NONE)] bleed: Prop<Bleed>,
    on_change: Callback<ScrollPosition>,
    children: Children<ListChild>,
) -> NodeId {
    let theme = use_theme();
    let bounded = !matches!(max_length, Prop::Static(None));
    let scroll = view! {
        <unstyled::Scroll
            api
            offset
            reveal
            direction
            fit={fit || bounded}
            follow_end
            bleed={bleed.map(|bleed| bleed.reach)}
            focus_ring={create_memo(move || Some(Outline::new(theme.accent.get(), FOCUS_RING_WIDTH.px()).offset(FOCUS_RING_OFFSET.px())))}
            scrollbar={scrollbar_style()}
            marker={|axis: Memo<Direction>| view! {
                <AutoscrollMarker axis />
            }}
            on_change={on_change}
        >
            {children}
        </unstyled::Scroll>
    };
    if !bounded {
        return scroll;
    }
    let length = use_size(max_length.map(|length| length.unwrap_or(Size::Custom(0.0))));
    let bound = create_memo(move || match max_length.get() {
        Some(_) => ItemSize::Intrinsic.max(length.get()),
        None => ItemSize::Intrinsic.into(),
    });
    view! {
        <List direction={direction} spacing=Space::None>{scroll} @sizing={bound}</List>
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
            radius=Radius::Full
            fill=ColorRole::SurfaceRaised
            border={Border::DEFAULT}
        >
            <List align=Align::Center spacing=Space::None>
                <Spacer @sizing=ItemSize::Percent(50.0) />
                <Text align=TextAlign::Center>
                    <Icon glyph={glyph} />
                </Text>
                <Spacer @sizing=ItemSize::Percent(50.0) />
            </List>
        </Frame>
    }
}

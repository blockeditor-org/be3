use beui_macros::{component, view};

use crate::base::{Align, Direction, ItemSize, ScrollPosition};
use crate::color::Color32;
use crate::icons::{ICON_HEIGHT, ICON_WIDTH};
use crate::node::NodeId;
use crate::reactive::{Callback, Children, Frame, List, Memo, Prop, Spacer, create_memo};
use crate::styled::Scrollbar;
use crate::styled::text::IconSized;
use crate::styled::theme::{
    BORDER_WIDTH, ICON_SIZE, SCROLLBAR_SPACING, SCROLLBAR_WIDTH, use_theme,
};
use crate::unstyled;
use crate::unstyled::{ScrollHandle, ScrollbarStyle};

const MARKER_SIZE: f32 = 28.0;
const MARKER_RADIUS: u8 = 14;

#[component]
pub fn Scroll(
    #[prop(default = 0.0)] offset: Prop<f32>,
    #[prop(default = None)] reveal: Prop<Option<usize>>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = None)] focus_color: Prop<Option<Color32>>,
    on_change: Callback<ScrollPosition>,
    children: Children<NodeId>,
) -> NodeId {
    let focus = focus_ring(focus_color);
    view! {
        <unstyled::Scroll
            offset
            reveal
            direction
            focus_color={focus}
            scrollbar={scrollbar_style()}
            marker={|axis: Memo<Direction>| view! {
                <AutoscrollMarker axis />
            }}
            on_change={move |reported: ScrollPosition| on_change.call(reported)}
        >
            {children}
        </unstyled::Scroll>
    }
}

pub(crate) fn scrollbar_style() -> ScrollbarStyle {
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
}

#[component]
fn AutoscrollMarker(axis: Prop<Direction>) -> NodeId {
    let theme = use_theme();
    let glyph = axis.map(|axis| {
        match axis {
            Direction::Vertical => ICON_HEIGHT,
            Direction::Horizontal => ICON_WIDTH,
        }
        .to_string()
    });
    view! {
        <Frame
            width={Some(MARKER_SIZE)}
            height={Some(MARKER_SIZE)}
            radius=MARKER_RADIUS
            color={theme.surface_raised.clone()}
            outline={theme.border.clone()}
            outline_width=BORDER_WIDTH
            outline_visible=true
        >
            <List align=Align::Center spacing=0.0>
                <Spacer @sizing=ItemSize::Percent(50.0) />
                <IconSized glyph font_size=ICON_SIZE color={theme.text.clone()} />
                <Spacer @sizing=ItemSize::Percent(50.0) />
            </List>
        </Frame>
    }
}

fn focus_ring(color: Prop<Option<Color32>>) -> Memo<Color32> {
    let theme = use_theme();
    create_memo(move || color.get().unwrap_or_else(|| theme.accent.get()))
}

use beui_macros::{component, view};

use crate::color::Color32;

use crate::base::{Direction, ScrollPosition};
use crate::node::NodeId;
use crate::reactive::{Callback, Frame, ItemSize, List, Prop, Spacer, clone, create_memo};
use crate::styled::theme::{ThemeStore, use_theme};
use crate::unstyled;
use crate::unstyled::{ScrollbarHandle, thumb_length, thumb_start};

const RADIUS: u8 = 3;

#[component]
pub fn Scrollbar(
    position: Prop<ScrollPosition>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    on_scroll_to: Callback<f32>,
) -> NodeId {
    view! {
        <unstyled::Scrollbar
            position
            direction
            on_scroll_to={move |offset: f32| on_scroll_to.call(offset)}
        >
            {move |handle: ScrollbarHandle| {
                view! {
                    <ScrollbarTrack handle />
                }
            }}
        </unstyled::Scrollbar>
    }
}

#[component]
fn ScrollbarTrack(handle: ScrollbarHandle) -> NodeId {
    let ScrollbarHandle {
        position,
        direction,
        hovered,
        dragging,
    } = handle;
    let theme = use_theme();

    let before = create_memo(clone!(position -> move || before_percent(position.get())));
    let thumb = create_memo(clone!(position -> move || thumb_percent(position.get())));
    let after = create_memo(clone!(position -> move || after_percent(position.get())));
    let color = create_memo(clone!(theme position -> move || {
        thumb_color(&theme, position.get(), hovered.get(), dragging.get())
    }));
    let track = create_memo(clone!(theme -> move || track_color(&theme, position.get())));

    view! {
        <Frame color={track} radius=RADIUS>
            <List direction spacing=0.0>
                <Spacer @sizing={before} />
                <Frame @sizing={thumb} color radius=RADIUS></Frame>
                <Spacer @sizing={after} />
            </List>
        </Frame>
    }
}

fn before_percent(position: ScrollPosition) -> ItemSize {
    ItemSize::Percent(thumb_start(position) * 100.0)
}

fn thumb_percent(position: ScrollPosition) -> ItemSize {
    ItemSize::Percent(thumb_length(position) * 100.0)
}

fn after_percent(position: ScrollPosition) -> ItemSize {
    let rest = 1.0 - thumb_start(position) - thumb_length(position);
    ItemSize::Percent(rest.max(0.0) * 100.0)
}

fn thumb_color(
    theme: &ThemeStore,
    position: ScrollPosition,
    hovered: bool,
    dragging: bool,
) -> Color32 {
    if position.max_offset() <= 0.0 {
        return Color32::TRANSPARENT;
    }
    match (dragging, hovered) {
        (true, _) => theme.scroll_thumb_active.get(),
        (false, true) => theme.scroll_thumb_hover.get(),
        (false, false) => theme.scroll_thumb.get(),
    }
}

fn track_color(theme: &ThemeStore, position: ScrollPosition) -> Color32 {
    if position.max_offset() > 0.0 {
        theme.surface_raised.get()
    } else {
        Color32::TRANSPARENT
    }
}

use crate::frame::{ColorRole, Frame, Radius};
use beui_macros::{component, view};

use beui_core::color::Color32;

use crate::theme::{SCROLLBAR_INSET, ThemeStore, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{ScrollbarHandle, ScrollbarThumb};
use beui_core::base::{Direction, ScrollPosition};
use beui_core::node::NodeId;
use beui_view::reactive::*;

const RADIUS: f32 = 3.0;

#[component]
pub fn Scrollbar(
    position: Prop<ScrollPosition>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = 1.0)] visibility: Prop<f32>,
    on_scroll_to: Callback<f32>,
    on_engage: Callback<bool>,
) -> NodeId {
    let shown = create_memo(move || visibility.get() > 0.0);
    let ends_horizontal = inset_when(&direction, Direction::Horizontal);
    let ends_vertical = inset_when(&direction, Direction::Vertical);
    view! {
        <Frame
            padding={create_memo(move || Sides::symmetric(ends_horizontal.get(), ends_vertical.get()))}
        >
            <unstyled::Scrollbar
                position
                direction
                shown
                on_scroll_to={move |offset: f32| on_scroll_to.call(offset)}
                on_engage={move |engaged: bool| on_engage.call(engaged)}
            >
                {move |handle: ScrollbarHandle| {
                    view! {
                        <ScrollbarTrack handle visibility />
                    }
                }}
            </unstyled::Scrollbar>
        </Frame>
    }
}

fn inset_when(direction: &Prop<Direction>, along: Direction) -> Memo<f32> {
    let direction = *direction;
    create_memo(move || match direction.get() == along {
        true => SCROLLBAR_INSET,
        false => 0.0,
    })
}

fn outer_side_inset(direction: &Prop<Direction>, along: Direction) -> Memo<f32> {
    let direction = *direction;
    create_memo(move || match direction.get() == along {
        true => SCROLLBAR_INSET,
        false => 0.0,
    })
}

#[component]
fn ScrollbarTrack(handle: ScrollbarHandle, visibility: Prop<f32>) -> NodeId {
    let ScrollbarHandle {
        position,
        direction,
        hovered,
        dragging,
        engaged,
    } = handle;
    let theme = use_theme();

    let color = create_memo(move || {
        thumb_color(&theme, position.get(), hovered.get(), dragging.get())
            .scale_alpha(visibility.get())
    });
    let track = create_memo(move || {
        track_color(&theme, position.get(), engaged.get()).scale_alpha(visibility.get())
    });
    let right = outer_side_inset(&direction, Direction::Vertical);
    let bottom = outer_side_inset(&direction, Direction::Horizontal);

    view! {
        <Frame padding={create_memo(move || Sides::new(0.0, 0.0, right.get(), bottom.get()))}>
            <Frame fill={ColorRole::custom(track)} radius={Radius::custom(RADIUS)}>
                <ScrollbarThumb position direction color radius=RADIUS />
            </Frame>
        </Frame>
    }
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

fn track_color(theme: &ThemeStore, position: ScrollPosition, engaged: bool) -> Color32 {
    if engaged && position.max_offset() > 0.0 {
        theme.surface_raised.get()
    } else {
        Color32::TRANSPARENT
    }
}

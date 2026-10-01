use beui_macros::{component, view};

use beui_core::color::Color32;

use crate::theme::{RADIUS, ThemeStore, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::ButtonHandle;
use beui_core::node::NodeId;
use beui_view::reactive::{Child, ClickCallback, Frame, Prop, clone, create_memo, focus_ring};

const PADDING_HORIZONTAL: f32 = 8.0;
const PADDING_VERTICAL: f32 = 4.0;

#[component]
pub fn ListRow(
    children: Child,
    #[prop(default = false)] selected: Prop<bool>,
    on_click: ClickCallback,
    on_activate: ClickCallback,
) -> NodeId {
    let face = selected.clone();
    let activates = !on_activate.is_empty();
    view! {
        <unstyled::ListRow
            selected
            activates
            on_click={move || on_click.call()}
            on_activate={move || on_activate.call()}
        >
            {move |handle: ButtonHandle| view! {
                <ListRowFace handle selected={face}>{children}</ListRowFace>
            }}
        </unstyled::ListRow>
    }
}

#[component]
fn ListRowFace(handle: ButtonHandle, selected: Prop<bool>, children: Child) -> NodeId {
    let ButtonHandle {
        hovered,
        active,
        focused,
    } = handle;
    let theme = use_theme();
    let fill_color = create_memo(clone!(theme -> move || {
        background(&theme, selected.get(), hovered.get(), active.get())
    }));
    view! {
        <Frame
            color={fill_color}
            outline={theme.accent.clone()}
            outline_width=2.0
            radius=RADIUS
            outline_visible={focus_ring(focused)}
            padding_horizontal=PADDING_HORIZONTAL
            padding_vertical=PADDING_VERTICAL
        >
            {children}
        </Frame>
    }
}

fn background(theme: &ThemeStore, selected: bool, hovered: bool, active: bool) -> Color32 {
    match (selected, hovered, active) {
        (_, _, true) => theme.pressed.get(),
        (_, true, false) => theme.hover.get(),
        (true, false, false) => theme.accent_soft.get(),
        (false, false, false) => Color32::TRANSPARENT,
    }
}

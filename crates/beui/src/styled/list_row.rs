use beui_macros::{component, view};

use crate::color::Color32;

use crate::node::NodeId;
use crate::reactive::{Child, ClickCallback, Frame, clone, create_memo};
use crate::styled::theme::{RADIUS, Theme, use_theme};
use crate::unstyled::{Button, ButtonHandle};

const PADDING_HORIZONTAL: f32 = 8.0;
const PADDING_VERTICAL: f32 = 4.0;

#[component]
pub fn ListRow(children: Child, on_click: ClickCallback) -> NodeId {
    view! {
        <Button
            on_click={move || on_click.call()}
            content={move |handle| view! {
                <ListRowFace handle>
                    {children}
                </ListRowFace>
            }}
        />
    }
}

#[component]
fn ListRowFace(handle: ButtonHandle, children: Child) -> NodeId {
    let ButtonHandle {
        hovered,
        active,
        focused,
    } = handle;
    let theme = use_theme();
    let fill_color =
        create_memo(clone!(theme -> move || background(&theme.get(), hovered.get(), active.get())));
    view! {
        <Frame
            color={fill_color}
            outline={theme.pick(|theme| theme.accent)}
            outline_width=2.0
            radius=RADIUS
            outline_visible={focused}
            padding_horizontal=PADDING_HORIZONTAL
            padding_vertical=PADDING_VERTICAL
        >
            {children}
        </Frame>
    }
}

fn background(theme: &Theme, hovered: bool, active: bool) -> Color32 {
    match (hovered, active) {
        (_, true) => theme.pressed,
        (true, false) => theme.hover,
        (false, false) => Color32::TRANSPARENT,
    }
}

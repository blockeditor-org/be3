use beui_macros::{component, view};

use crate::color::Color32;

use crate::node::NodeId;
use crate::reactive::{create_memo, Child, ClickCallback, Frame};
use crate::styled::theme::{ACCENT, BORDER, RADIUS, SURFACE_RAISED};
use crate::unstyled::{Button, ButtonHandle};

const PADDING_HORIZONTAL: f32 = 8.0;
const PADDING_VERTICAL: f32 = 4.0;

#[component]
pub fn ListRow(children: Child, on_click: ClickCallback) -> NodeId {
    view! {
        <Button
            on_click={move || on_click.call()}
            content={move |handle| view! { <ListRowFace handle>{children}</ListRowFace> }}
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
    let fill_color = create_memo(move || background(hovered.get(), active.get()));
    view! {
        <Frame color={fill_color} outline=ACCENT outline_width=2.0 radius=RADIUS outline_visible={focused} padding_horizontal=PADDING_HORIZONTAL padding_vertical=PADDING_VERTICAL>
            {children}
        </Frame>
    }
}

fn background(hovered: bool, active: bool) -> Color32 {
    match (hovered, active) {
        (_, true) => BORDER,
        (true, false) => SURFACE_RAISED,
        (false, false) => Color32::TRANSPARENT,
    }
}

use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::color::Color32;
use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{create_memo, Callback, Frame, Prop, Text};
use crate::styled::theme::{
    ACCENT, ACCENT_SOFT, BORDER, FONT_BODY, RADIUS, SURFACE, SURFACE_RAISED, TEXT,
};
use crate::unstyled;
use crate::unstyled::{Toggle, ToggleHandle};

#[component]
pub fn ToggleButton(label: Prop<String>, pressed: Prop<bool>, on_change: Callback<bool>) -> NodeId {
    let label_text = create_memo(move || label.get());
    let accessibility = create_memo({
        let label_text = label_text.clone();
        move || {
            let label = label_text.get();
            let mut node = Node::new(Role::Button);
            node.set_label(label);
            node
        }
    });

    view! {
        <Toggle checked={pressed} accessibility on_change={move |pressed| on_change.call(pressed)}>
            {move |handle| view! { <ToggleButtonFace handle label={label_text} /> }}
        </Toggle>
    }
}

#[component]
fn ToggleButtonFace(handle: ToggleHandle, label: Prop<String>) -> NodeId {
    let ToggleHandle {
        checked,
        hovered,
        focused,
        ..
    } = handle;
    let fill_color = create_memo({
        let checked = checked.clone();
        move || fill_for(checked.get(), hovered.get())
    });
    let border_color = create_memo(move || if checked.get() { ACCENT } else { BORDER });

    view! {
        <Frame outline=ACCENT outline_width=2.0 radius=RADIUS outline_offset=3.0 outline_visible={focused}>
            <Frame color={fill_color} outline={border_color} outline_width=1.0 radius=RADIUS outline_visible=true padding_horizontal=14.0 padding_vertical=8.0>
                <Text string={label} font_size=FONT_BODY color=TEXT />
            </Frame>
        </Frame>
    }
}

fn fill_for(pressed: bool, hovered: bool) -> Color32 {
    if pressed {
        ACCENT_SOFT
    } else if hovered {
        SURFACE_RAISED
    } else {
        SURFACE
    }
}

pub fn toggle_button_pressed(document: &Document, button: NodeId) -> bool {
    unstyled::toggle_checked(document, button).get()
}

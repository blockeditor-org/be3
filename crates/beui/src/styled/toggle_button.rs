use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::color::Color32;
use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{Callback, Frame, Prop, Text, clone, create_memo};
use crate::styled::theme::{FONT_BODY, RADIUS, Theme, use_theme};
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
    let theme = use_theme();
    let fill_color = create_memo(
        clone!(checked theme -> move || fill_for(&theme.get(), checked.get(), hovered.get())),
    );
    let border_color = create_memo(clone!(theme -> move || {
        let theme = theme.get();
        if checked.get() {
            theme.accent
        } else {
            theme.border
        }
    }));

    view! {
        <Frame outline={theme.pick(|theme| theme.accent)} outline_width=2.0 radius=RADIUS outline_offset=3.0 outline_visible={focused}>
            <Frame color={fill_color} outline={border_color} outline_width=1.0 radius=RADIUS outline_visible=true padding_horizontal=14.0 padding_vertical=8.0>
                <Text string={label} font_size=FONT_BODY color={theme.pick(|theme| theme.text)} />
            </Frame>
        </Frame>
    }
}

fn fill_for(theme: &Theme, pressed: bool, hovered: bool) -> Color32 {
    if pressed {
        theme.accent_soft
    } else if hovered {
        theme.hover
    } else {
        theme.surface
    }
}

pub fn toggle_button_pressed(document: &Document, button: NodeId) -> bool {
    unstyled::toggle_checked(document, button).get()
}

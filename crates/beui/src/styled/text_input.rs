use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::color::Color32;

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{clone, create_memo, Callback, Frame, Prop};
use crate::styled::theme::{use_theme, Theme, BORDER_WIDTH, FONT_BODY, RADIUS};
use crate::unstyled;
use crate::unstyled::TextInputHandle;

const HEIGHT: f32 = 34.0;
const PADDING_HORIZONTAL: f32 = 10.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 3.0;

#[component]
pub fn TextInput(
    value: Prop<String>,
    placeholder: Prop<String>,
    #[prop(default = String::new())] label: Prop<String>,
    on_change: Callback<String>,
    on_submit: Callback<String>,
) -> NodeId {
    let accessibility = label.map(|label| {
        let mut node = Node::new(Role::TextInput);
        if !label.is_empty() {
            node.set_label(label);
        }
        node
    });
    let theme = use_theme();
    view! {
        <unstyled::TextInput
            value
            placeholder
            accessibility
            font_size=FONT_BODY
            color={theme.pick(|theme| theme.text)}
            placeholder_color={theme.pick(|theme| theme.text_muted)}
            selection_color={theme.pick(|theme| theme.accent_soft)}
            caret_color={theme.pick(|theme| theme.accent)}
            padding_horizontal=PADDING_HORIZONTAL
            on_change={move |value| on_change.call(value)}
            on_submit={move |value| on_submit.call(value)}
        >
            {move |handle| view! { <TextInputFrame handle /> }}
        </unstyled::TextInput>
    }
}

#[component]
fn TextInputFrame(handle: TextInputHandle) -> NodeId {
    let TextInputHandle {
        field,
        hovered,
        focused,
    } = handle;
    let theme = use_theme();
    let border = create_memo(
        clone!(focused theme -> move || border_color(&theme.get(), focused.get(), hovered.get())),
    );
    view! {
        <Frame outline={theme.pick(|theme| theme.accent)} outline_width=FOCUS_RING_WIDTH radius=RADIUS outline_offset=FOCUS_RING_OFFSET outline_visible={focused}>
            <Frame height=HEIGHT color={theme.pick(|theme| theme.surface_raised)} outline={border} outline_width=BORDER_WIDTH radius=RADIUS outline_visible=true>
                {field}
            </Frame>
        </Frame>
    }
}

pub fn text_input_value(document: &Document, input: NodeId) -> String {
    unstyled::text_input_value(document, input)
}

fn border_color(theme: &Theme, focused: bool, hovered: bool) -> Color32 {
    match (focused, hovered) {
        (true, _) => theme.accent,
        (false, true) => theme.text_muted,
        (false, false) => theme.border,
    }
}

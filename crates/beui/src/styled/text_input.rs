use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::color::Color32;

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{Callback, Frame, Prop, clone, create_memo};
use crate::styled::context_menu::text_input_menu;
use crate::styled::theme::{BORDER_WIDTH, FONT_BODY, RADIUS, ThemeStore, use_theme};
use crate::unstyled;
use crate::unstyled::TextInputHandle;

const HEIGHT: f32 = 34.0;
const PADDING_HORIZONTAL: f32 = 10.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 3.0;

#[component]
pub fn TextInput(
    value: Prop<String>,
    #[prop(default = String::new())] placeholder: Prop<String>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] focused: Prop<bool>,
    on_change: Callback<String>,
    on_submit: Callback<String>,
    on_focus_change: Callback<bool>,
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
            disabled
            focused
            accessibility
            font_size=FONT_BODY
            color={theme.text.clone()}
            placeholder_color={theme.text_muted.clone()}
            selection_color={theme.accent_soft.clone()}
            caret_color={theme.accent.clone()}
            padding_horizontal=PADDING_HORIZONTAL
            menu={text_input_menu()}
            on_change={move |value| on_change.call(value)}
            on_submit={move |value| on_submit.call(value)}
            on_focus_change={move |focused| on_focus_change.call(focused)}
        >
            {move |handle| view! {
                <TextInputFrame handle />
            }}
        </unstyled::TextInput>
    }
}

#[component]
fn TextInputFrame(handle: TextInputHandle) -> NodeId {
    let TextInputHandle {
        field,
        hovered,
        focused,
        disabled,
    } = handle;
    let theme = use_theme();
    let border = create_memo(clone!(focused theme disabled -> move || {
        border_color(&theme, disabled.get(), focused.get(), hovered.get())
    }));
    let fill = create_memo(clone!(theme disabled -> move || match disabled.get() {
        true => theme.surface.get(),
        false => theme.surface_raised.get(),
    }));
    view! {
        <Frame
            outline={theme.accent.clone()}
            outline_width=FOCUS_RING_WIDTH
            radius=RADIUS
            outline_offset=FOCUS_RING_OFFSET
            outline_visible={focused}
        >
            <Frame
                height=HEIGHT
                color={fill}
                outline={border}
                outline_width=BORDER_WIDTH
                radius=RADIUS
                outline_visible=true
            >
                {field}
            </Frame>
        </Frame>
    }
}

pub fn text_input_value(document: &Document, input: NodeId) -> String {
    unstyled::text_input_value(document, input)
}

fn border_color(theme: &ThemeStore, disabled: bool, focused: bool, hovered: bool) -> Color32 {
    if disabled {
        return theme.border.get();
    }
    match (focused, hovered) {
        (true, _) => theme.accent.get(),
        (false, true) => theme.text_muted.get(),
        (false, false) => theme.border.get(),
    }
}

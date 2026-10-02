use accesskit::{Node, Role};
use beui_macros::{component, view};

use beui_core::color::Color32;

use crate::context_menu::menu_style;
use crate::theme::{BORDER_WIDTH, FONT_BODY, RADIUS, field_border, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{TextInputHandle, TextInputStyle};
use beui_core::document::Document;
use beui_core::icons::ICON_CLOSE;
use beui_core::input::KeyPress;
use beui_core::node::NodeId;
use beui_view::reactive::{
    Align, Callback, ClickCallback, Direction, Frame, IntoProp, ItemSize, List, Memo, Prop, Show,
    clone, create_memo,
};

use crate::icon_button::{IconButton, IconButtonSize};
use crate::text::Icon;

const HEIGHT: f32 = 34.0;
const PADDING_HORIZONTAL: f32 = 10.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 3.0;
const CLEAR_PADDING: f32 = 6.0;

#[component]
pub fn TextInput(
    value: Prop<String>,
    #[prop(default = String::new())] placeholder: Prop<String>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = false)] password: Prop<bool>,
    #[prop(default = false)] plain: Prop<bool>,
    #[prop(default = false)] select_on_focus: Prop<bool>,
    #[prop(default = String::new())] glyph: Prop<String>,
    #[prop(default = false)] clearable: Prop<bool>,
    on_change: Callback<String>,
    on_submit: Callback<String>,
    on_focus_change: Callback<bool>,
    on_key_override: Callback<KeyPress, bool>,
) -> NodeId {
    let accessibility = label.map(|label| {
        let mut node = Node::new(Role::TextInput);
        if !label.is_empty() {
            node.set_label(label);
        }
        node
    });
    let plain = create_memo(move || plain.get());
    let glyph = create_memo(move || glyph.get());
    let current = value.clone();
    let clearable = create_memo(move || clearable.get() && !current.get().is_empty());
    let clear = on_change.clone();
    view! {
        <unstyled::TextInput
            value
            placeholder
            disabled
            focused
            password
            select_on_focus
            accessibility
            style={text_input_style()}
            on_change={move |value| on_change.call(value)}
            on_submit={move |value| on_submit.call(value)}
            on_focus_change={move |focused| on_focus_change.call(focused)}
            on_key_override={move |press| on_key_override.call(press)}
        >
            {move |handle| view! {
                <TextInputFrame
                    handle
                    plain={plain.clone()}
                    glyph={glyph.clone()}
                    clearable={clearable.clone()}
                    on_clear={clone!(clear -> move || clear.call(String::new()))}
                />
            }}
        </unstyled::TextInput>
    }
}

#[component]
fn TextInputFrame(
    handle: TextInputHandle,
    plain: Memo<bool>,
    glyph: Memo<String>,
    clearable: Memo<bool>,
    on_clear: ClickCallback,
) -> NodeId {
    let TextInputHandle {
        field,
        hovered,
        focused,
        disabled,
    } = handle;
    let theme = use_theme();
    let marked = create_memo(clone!(glyph -> move || !glyph.get().is_empty()));
    let raised = create_memo(clone!(focused hovered -> move || {
        !plain.get() || focused.get() || hovered.get()
    }));
    let border = create_memo(clone!(focused theme disabled -> move || {
        field_border(&theme, disabled.get(), focused.get(), hovered.get())
    }));
    let fill = create_memo(clone!(theme disabled raised -> move || {
        match (raised.get(), disabled.get()) {
            (false, _) => Color32::TRANSPARENT,
            (true, true) => theme.surface.get(),
            (true, false) => theme.surface_raised.get(),
        }
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
                outline_visible={raised}
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=0.0>
                    <Show condition={marked}>
                        {move || clone!(glyph theme -> view! {
                            <Frame padding_left=PADDING_HORIZONTAL>
                                <Icon glyph={glyph.clone()} color={theme.text_muted.clone()} />
                            </Frame>
                        })}
                    </Show>
                    {field} @sizing=ItemSize::Percent(100.0)
                    <Show condition={clearable}>
                        {move || clone!(on_clear -> view! {
                            <Frame padding_horizontal=CLEAR_PADDING>
                                <IconButton
                                    glyph={ICON_CLOSE.to_owned()}
                                    label="Clear"
                                    size=IconButtonSize::Compact
                                    press_focus=false
                                    on_click={move || on_clear.call()}
                                />
                            </Frame>
                        })}
                    </Show>
                </List>
            </Frame>
        </Frame>
    }
}

pub fn text_input_style() -> TextInputStyle {
    let theme = use_theme();
    TextInputStyle {
        font_size: Prop::Static(FONT_BODY),
        color: theme.text.clone().into_prop(),
        placeholder_color: theme.text_muted.clone().into_prop(),
        selection_color: theme.accent_soft.clone().into_prop(),
        caret_color: theme.accent.clone().into_prop(),
        padding_horizontal: Prop::Static(PADDING_HORIZONTAL),
        padding_vertical: Prop::Static(0.0),
        menu: menu_style(),
    }
}

pub fn text_input_value(document: &Document, input: NodeId) -> String {
    unstyled::text_input_value(document, input)
}

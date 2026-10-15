use beui_macros::{component, view};

use beui_core::color::Color32;

use crate::context_menu::menu_style;
use crate::theme::{BORDER_WIDTH, FOCUS_RING_WIDTH, RADIUS, field_border, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{TextInputHandle, TextInputStyle};
use beui_core::document::Document;
use beui_core::icons::ICON_CLOSE;
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

use crate::icon_button::{IconButton, IconButtonSize};
use crate::text::{Icon, Text, TextColor, TextSize};
use beui_core::font::TextAlign;

const HEIGHT: f32 = 34.0;
const PADDING_HORIZONTAL: f32 = 10.0;
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
) -> NodeId {
    let plain = create_memo(move || plain.get());
    let glyph = create_memo(move || glyph.get());
    let clearable = create_memo(move || clearable.get());
    view! {
        <unstyled::TextInput
            value
            placeholder
            disabled
            focused
            password
            select_on_focus
            label
            style={text_input_style()}
            on_change={on_change}
            on_submit={on_submit}
            on_focus_change={on_focus_change}
        >
            {move |handle| view! {
                <TextInputFrame handle plain={plain} glyph={glyph} clearable={clearable} />
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
) -> NodeId {
    let TextInputHandle {
        field,
        hovered,
        focused,
        disabled,
        empty,
        clear,
    } = handle;
    let clearable = create_memo(move || clearable.get() && !empty.get());
    let theme = use_theme();
    let marked = create_memo(move || !glyph.get().is_empty());
    let raised = create_memo(move || !plain.get() || focused.get() || hovered.get());
    let border =
        create_memo(move || field_border(&theme, disabled.get(), focused.get(), hovered.get()));
    let fill = create_memo(move || match (raised.get(), disabled.get()) {
        (false, _) => Color32::TRANSPARENT,
        (true, true) => theme.surface.get(),
        (true, false) => theme.surface_raised.get(),
    });
    view! {
        <Frame
            outline={create_memo(move || focused.get().then(|| Outline::new(theme.accent.get(), FOCUS_RING_WIDTH).offset(FOCUS_RING_OFFSET)))}
            radius=RADIUS
        >
            <Frame
                height=HEIGHT
                fill=fill
                outline={create_memo(move || raised.get().then(|| Outline::new(border.get(), BORDER_WIDTH)))}
                radius=RADIUS
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=0.0>
                    <Show condition={marked}>
                        {move || view! {
                            <Frame padding={Sides::new(PADDING_HORIZONTAL, 0.0, 0.0, 0.0)}>
                                <Text color=TextColor::Muted align=TextAlign::Center>
                                    <Icon glyph={glyph} />
                                </Text>
                            </Frame>
                        }}
                    </Show>
                    {field} @sizing=ItemSize::Percent(100.0)
                    <Show condition={clearable}>
                        {move || view! {
                            <Frame padding={Sides::horizontal(CLEAR_PADDING)}>
                                <IconButton
                                    glyph={ICON_CLOSE.to_owned()}
                                    label="Clear"
                                    size=IconButtonSize::Compact
                                    press_focus=false
                                    on_click={clear}
                                />
                            </Frame>
                        }}
                    </Show>
                </List>
            </Frame>
        </Frame>
    }
}

pub fn text_input_style() -> TextInputStyle {
    let theme = use_theme();
    TextInputStyle {
        font_size: Prop::Static(TextSize::Base.px()),
        color: theme.text.into_prop(),
        placeholder_color: theme.text_muted.into_prop(),
        selection_color: theme.accent_soft.into_prop(),
        caret_color: theme.accent.into_prop(),
        padding_horizontal: Prop::Static(PADDING_HORIZONTAL),
        padding_vertical: Prop::Static(0.0),
        menu: menu_style(),
    }
}

pub fn text_input_value(document: &Document, input: NodeId) -> String {
    unstyled::text_input_value(document, input)
}

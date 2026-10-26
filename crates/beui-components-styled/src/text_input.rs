use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_core::icons::Glyph;
use beui_macros::{component, view};

use beui_core::color::Color32;

use crate::context_menu::menu_style;
use crate::theme::{field_border, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{TextInputHandle, TextInputStyle};
use beui_core::icons::{ICON_COPY, ICON_X};
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

use crate::button::{Button, ButtonSize};
use crate::text::{Icon, Text, TextColor, TextSize};
use beui_core::font::TextAlign;

const HEIGHT: f32 = 34.0;
const MULTILINE_HEIGHT: f32 = 104.0;
const PADDING_VERTICAL: f32 = 8.0;
const PADDING_HORIZONTAL: f32 = 10.0;
const FOCUS_RING_OFFSET: f32 = 3.0;
const CLEAR_PADDING: f32 = 6.0;

#[component]
pub fn TextInput(
    #[prop(default = ComponentRef::new())] api: ComponentRef<crate::TextInputApi>,

    value: Binding<String>,
    #[prop(default = String::new())] placeholder: Prop<String>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = false)] password: Prop<bool>,
    #[prop(default = false)] plain: Prop<bool>,
    #[prop(default = false)] select_on_focus: Prop<bool>,
    #[prop(default = None)] glyph: Prop<Option<Glyph>>,
    #[prop(default = false)] clearable: Prop<bool>,
    #[prop(default = false)] copyable: Prop<bool>,
    #[prop(default = false)] multiline: bool,
    on_submit: Callback<String>,
    on_focus_change: Callback<bool>,
) -> NodeId {
    let mut style = text_input_style();
    if multiline {
        style.padding_vertical = Prop::Static(PADDING_VERTICAL);
    }
    let plain = create_memo(move || plain.get());
    let glyph = create_memo(move || glyph.get());
    let clearable = create_memo(move || clearable.get());
    let copyable = create_memo(move || copyable.get());
    let copied = create_memo(move || value.get());
    let named = label.clone();
    let copy_label = create_memo(move || match named.get().is_empty() {
        true => "Copy".to_owned(),
        false => format!("Copy {}", named.get()),
    });
    view! {
        <unstyled::TextInput
            api
            value
            placeholder
            disabled
            focused
            password
            select_on_focus
            label
            multiline
            style={style}
            on_submit={on_submit}
            on_focus_change={on_focus_change}
        >|handle|
            <TextInputFrame
                handle
                plain={plain}
                glyph={glyph}
                clearable={clearable}
                copyable={copyable}
                copied={copied}
                copy_label={copy_label}
                multiline
            />
        </unstyled::TextInput>
    }
}

#[component]
fn TextInputFrame(
    handle: TextInputHandle,
    plain: Memo<bool>,
    glyph: Memo<Option<Glyph>>,
    clearable: Memo<bool>,
    copyable: Memo<bool>,
    copied: Memo<String>,
    copy_label: Memo<String>,
    multiline: bool,
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
    let marked = create_memo(move || glyph.get().is_some());
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
            border={create_memo(move || focused.get().then_some(Border::FOCUS.offset(FOCUS_RING_OFFSET)))}
            radius=Radius::Md
        >
            <Frame
                height={match multiline {
                    true => MULTILINE_HEIGHT,
                    false => HEIGHT,
                }}
                fill={ColorRole::custom(fill)}
                border={create_memo(move || raised.get().then(|| Border::new(ColorRole::Custom(border.get()))))}
                radius=Radius::Md
            >
                <List
                    direction=Direction::Horizontal
                    align={match multiline {
                        true => Align::Stretch,
                        false => Align::Center,
                    }}
                    spacing=0.0
                >
                    <Show condition={marked}>
                        <Frame padding={Sides::new(PADDING_HORIZONTAL, 0.0, 0.0, 0.0)}>
                            <Text color=TextColor::Muted align=TextAlign::Center>
                                <Icon glyph={glyph} />
                            </Text>
                        </Frame>
                    </Show>
                    {field} @sizing=ItemSize::Percent(100.0)
                    <Show condition={clearable}>
                        <Frame padding={Sides::horizontal(CLEAR_PADDING)}>
                            <Button
                                icon_only=true
                                glyph=ICON_X
                                label="Clear"
                                size=ButtonSize::Small
                                press_focus=false
                                capture_presses=true
                                on_click={clear}
                            />
                        </Frame>
                    </Show>
                    <Show condition={copyable}>
                        <Frame padding={Sides::horizontal(CLEAR_PADDING)}>
                            <Button
                                icon_only=true
                                glyph=ICON_COPY
                                label={copy_label}
                                size=ButtonSize::Small
                                press_focus=false
                                capture_presses=true
                                on_click={move || copy_text(copied.get_untracked())}
                            />
                        </Frame>
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
        prose: Prop::Static(None),
        menu: menu_style(),
    }
}

use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_macros::{component, view};
use beui_view::types::icons::Glyph;

use crate::context_menu::menu_style;
use crate::text_area::text_area_geometry;
use crate::theme::{field_border, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{TextInputHandle, TextInputStyle};
use beui_view::reactive::NodeId;
use beui_view::types::icons::{ICON_COPY, ICON_X};

use crate::list::List;
use crate::tokens::{Padding, Size, Space, Stroke, use_space};
use beui_view::reactive::*;

use crate::button::{Button, ButtonSize};
use crate::text::{Icon, Text, TextColor, TextSize};
use beui_view::types::TextAlign;

const HEIGHT: Size = Size::Field;
const MULTILINE_HEIGHT: Size = Size::Custom(104.0);
const PADDING_VERTICAL: Space = Space::Md;
const PADDING_HORIZONTAL: Space = Space::Md;
const CLEAR_PADDING: Space = Space::Sm;

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
        style.padding_vertical = use_space(PADDING_VERTICAL);
    }
    let plain = create_memo(move || plain.get());
    let glyph = create_memo(move || glyph.get());
    let clearable = create_memo(move || clearable.get());
    let copyable = create_memo(move || copyable.get());
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
        copy,
    } = handle;
    let clearable = create_memo(move || clearable.get() && !empty.get());
    let marked = create_memo(move || glyph.get().is_some());
    let raised = create_memo(move || !plain.get() || focused.get() || hovered.get());
    let border = create_memo(move || {
        raised
            .get()
            .then(|| Border::new(field_border(disabled.get(), focused.get(), hovered.get())))
    });
    let fill = create_memo(move || match (raised.get(), disabled.get()) {
        (false, _) => ColorRole::None,
        (true, true) => ColorRole::Surface,
        (true, false) => ColorRole::SurfaceRaised,
    });
    view! {
        <Frame
            border={create_memo(move || focused.get().then_some(Border::FOCUS.offset(Stroke::Focus)))}
            radius=Radius::Md
        >
            <Frame
                height={match multiline {
                    true => MULTILINE_HEIGHT,
                    false => HEIGHT,
                }}
                fill={fill}
                border={border}
                radius=Radius::Md
            >
                <List
                    direction=Direction::Horizontal
                    align={match multiline {
                        true => Align::Stretch,
                        false => Align::Center,
                    }}
                    spacing=Space::None
                >
                    <Show condition={marked}>
                        <Frame padding={Padding::ZERO.left(PADDING_HORIZONTAL)}>
                            <Text color=TextColor::Muted align=TextAlign::Center>
                                <Icon glyph={glyph} />
                            </Text>
                        </Frame>
                    </Show>
                    {field} @sizing=ItemSize::Percent(100.0)
                    <Show condition={clearable}>
                        <Frame padding={Padding::horizontal(CLEAR_PADDING)}>
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
                        <Frame padding={Padding::horizontal(CLEAR_PADDING)}>
                            <Button
                                icon_only=true
                                glyph=ICON_COPY
                                label={copy_label}
                                size=ButtonSize::Small
                                press_focus=false
                                capture_presses=true
                                on_click={copy}
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
        padding_horizontal: use_space(PADDING_HORIZONTAL),
        padding_vertical: Prop::Static(0.0),
        prose: Prop::Static(None),
        geometry: Prop::Static(text_area_geometry()),
        menu: Some(menu_style()),
    }
}

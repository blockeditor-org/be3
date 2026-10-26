use crate::frame::{Border, ColorRole, Frame, Radius, focus_border};
use beui_core::font::Overflow;
use beui_macros::{component, view};

use crate::scroll::scrollbar_style;
use crate::text::{Icon, Text, TextColor};
use crate::text_input::text_input_style;
use crate::theme::{ThemeStore, field_border, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{
    ChoiceOption, SelectOptionHandle, SelectTriggerHandle, TextInputHandle,
};
use beui_core::base::TextAlign;
use beui_core::color::Color32;
use beui_core::icons::ICON_CHEVRON_DOWN;
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const TRIGGER_MAX_WIDTH: f32 = 220.0;
const POPUP_WIDTH: f32 = 220.0;
const POPUP_PADDING: f32 = 6.0;
const OPTIONS_MAX_HEIGHT: f32 = 240.0;
const HEIGHT: f32 = 34.0;
const PADDING_HORIZONTAL: f32 = 10.0;
const OPTION_PADDING_VERTICAL: f32 = 6.0;
const FOCUS_RING_OFFSET: f32 = 3.0;
const ARROW_SPACING: f32 = 4.0;

#[component]
pub fn Select(
    #[prop(default = ComponentRef::new())] api: ComponentRef<crate::SelectApi>,

    options: Children<ChoiceOption>,
    #[prop(default = None)] selected: Binding<Option<usize>>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = String::new())] placeholder: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = Extent::at_most(TRIGGER_MAX_WIDTH))] width: Prop<Extent>,
) -> NodeId {
    let options = options.into_run();
    let placeholder = create_memo(move || placeholder.get());
    view! {
        <unstyled::Select
            api
            options
            selected
            disabled
            label
            search_placeholder="Search"
            search_content={|handle| view! {
                <SearchField handle />
            }}
            search_style={text_input_style()}
            scrollbar={scrollbar_style()}
            list_height=OPTIONS_MAX_HEIGHT
            spacing=POPUP_PADDING
            trigger={move |handle| view! {
                <SelectTrigger handle placeholder={placeholder} width={width} />
            }}
            option={|handle| view! {
                <SelectOption handle />
            }}
        >|content|
            <SelectPopup>{content}</SelectPopup>
        </unstyled::Select>
    }
}

#[component]
fn SelectTrigger(
    handle: SelectTriggerHandle,
    placeholder: Memo<String>,
    width: Prop<Extent>,
) -> NodeId {
    let SelectTriggerHandle {
        label,
        hovered,
        focused,
        disabled,
        ..
    } = handle;
    let theme = use_theme();
    let empty = create_memo(move || label.with(Option::is_none));
    let label_text = create_memo(move || {
        label.get().unwrap_or_else(|| match placeholder.get() {
            placeholder if placeholder.is_empty() => "Select...".to_owned(),
            placeholder => placeholder,
        })
    });
    let border =
        create_memo(move || field_border(&theme, disabled.get(), focused.get(), hovered.get()));
    let fill = create_memo(move || match disabled.get() {
        true => theme.surface.get(),
        false => theme.surface_raised.get(),
    });
    let text_color = create_memo(move || match disabled.get() || empty.get() {
        true => TextColor::Muted,
        false => TextColor::Default,
    });
    let arrow_color = create_memo(move || match disabled.get() {
        true => TextColor::Muted,
        false => TextColor::Default,
    });
    view! {
        <Frame
            width={width}
            border={focus_border(focused, Border::FOCUS.offset(FOCUS_RING_OFFSET))}
            radius=Radius::Md
        >
            <Frame
                height=HEIGHT
                fill={ColorRole::custom(fill)}
                border={create_memo(move || Some(Border::new(ColorRole::Custom(border.get()))))}
                radius=Radius::Md
                padding={Sides::horizontal(PADDING_HORIZONTAL)}
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=ARROW_SPACING>
                    <Text
                        color={text_color}
                        align=TextAlign::Start
                        overflow=Overflow::Ellipsis
                        @sizing=ItemSize::Percent(100.0)
                    >
                        {label_text}
                    </Text>
                    <Text color={arrow_color} align=TextAlign::Center>
                        <Icon glyph=ICON_CHEVRON_DOWN />
                    </Text>
                </List>
            </Frame>
        </Frame>
    }
}

#[component]
fn SearchField(handle: TextInputHandle) -> NodeId {
    let TextInputHandle {
        field,
        hovered,
        focused,
        ..
    } = handle;
    let theme = use_theme();
    let border = create_memo(move || field_border(&theme, false, focused.get(), hovered.get()));
    view! {
        <Frame
            height=HEIGHT
            fill=ColorRole::Surface
            border={create_memo(move || Some(Border::new(ColorRole::Custom(border.get()))))}
            radius=Radius::Md
        >
            {field}
        </Frame>
    }
}

#[component]
fn SelectOption(handle: SelectOptionHandle) -> NodeId {
    let SelectOptionHandle {
        label,
        highlighted,
        hovered,
        ..
    } = handle;
    let theme = use_theme();
    let fill_color =
        create_memo(move || option_background(&theme, highlighted.get(), hovered.get()));
    view! {
        <Frame
            fill={ColorRole::custom(fill_color)}
            radius=Radius::Md
            padding={Sides::symmetric(PADDING_HORIZONTAL, OPTION_PADDING_VERTICAL)}
        >
            <Text overflow=Overflow::Ellipsis align=TextAlign::Start>{label}</Text>
        </Frame>
    }
}

#[component]
fn SelectPopup(children: Child) -> NodeId {
    view! {
        <Frame
            width=POPUP_WIDTH
            fill=ColorRole::SurfaceRaised
            border={Border::DEFAULT}
            radius=Radius::Md
            padding=POPUP_PADDING
        >
            {children}
        </Frame>
    }
}

fn option_background(theme: &ThemeStore, highlighted: bool, hovered: bool) -> Color32 {
    match (highlighted, hovered) {
        (true, _) => theme.accent_soft.get(),
        (false, true) => theme.surface.get(),
        (false, false) => Color32::TRANSPARENT,
    }
}

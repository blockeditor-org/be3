use beui_macros::{component, view};

use crate::scroll::scrollbar_style;
use crate::text::Icon;
use crate::text_input::text_input_style;
use crate::theme::{
    BORDER_WIDTH, FOCUS_RING_WIDTH, FONT_BODY, RADIUS, ThemeStore, field_border, use_theme,
};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{
    ChoiceOption, SelectOptionHandle, SelectTriggerHandle, TextInputHandle,
};
use beui_core::base::TextAlign;
use beui_core::color::Color32;
use beui_core::document::Document;
use beui_core::icons::ICON_ARROW_DROP_DOWN;
use beui_core::node::NodeId;
use beui_view::reactive::{
    Align, Callback, Child, Children, Direction, Frame, ItemSize, List, Memo, Prop, Text,
    create_memo, focus_ring,
};

const TRIGGER_MAX_WIDTH: f32 = 220.0;
const POPUP_WIDTH: f32 = 220.0;
const POPUP_PADDING: f32 = 6.0;
const HEIGHT: f32 = 34.0;
const PADDING_HORIZONTAL: f32 = 10.0;
const OPTION_PADDING_VERTICAL: f32 = 6.0;
const FOCUS_RING_OFFSET: f32 = 3.0;
const ARROW_SPACING: f32 = 4.0;

#[component]
pub fn Select(
    options: Children<ChoiceOption>,
    selected: Prop<Option<usize>>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = String::new())] placeholder: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    on_change: Callback<Option<usize>>,
) -> NodeId {
    let options = options.into_run();
    let placeholder = create_memo(move || placeholder.get());
    view! {
        <unstyled::Select
            options
            selected
            disabled
            label
            on_change={move |selected| on_change.call(selected)}
            search_placeholder="Search"
            search_content={|handle| view! {
                <SearchField handle />
            }}
            search_style={text_input_style()}
            scrollbar={scrollbar_style()}
            trigger={move |handle| view! {
                <SelectTrigger handle placeholder={placeholder} />
            }}
            option={|handle| view! {
                <SelectOption handle />
            }}
        >
            {|content| view! {
                <SelectPopup>{content}</SelectPopup>
            }}
        </unstyled::Select>
    }
}

#[component]
fn SelectTrigger(handle: SelectTriggerHandle, placeholder: Memo<String>) -> NodeId {
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
        true => theme.text_muted.get(),
        false => theme.text.get(),
    });
    let arrow_color = create_memo(move || match disabled.get() {
        true => theme.text_muted.get(),
        false => theme.text.get(),
    });
    view! {
        <Frame
            max_width=TRIGGER_MAX_WIDTH
            outline={theme.accent}
            outline_width=FOCUS_RING_WIDTH
            radius=RADIUS
            outline_offset=FOCUS_RING_OFFSET
            outline_visible={focus_ring(focused)}
        >
            <Frame
                height=HEIGHT
                color={fill}
                outline={border}
                outline_width=BORDER_WIDTH
                radius=RADIUS
                outline_visible=true
                padding_horizontal=PADDING_HORIZONTAL
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=ARROW_SPACING>
                    <Text
                        string={label_text}
                        font_size=FONT_BODY
                        color={text_color}
                        align=TextAlign::Start
                        ellipsis=true
                        @sizing=ItemSize::Percent(100.0)
                    />
                    <Icon glyph={ICON_ARROW_DROP_DOWN.to_owned()} color={arrow_color} />
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
            color={theme.surface}
            outline={border}
            outline_width=BORDER_WIDTH
            radius=RADIUS
            outline_visible=true
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
            color={fill_color}
            radius=RADIUS
            padding_horizontal=PADDING_HORIZONTAL
            padding_vertical=OPTION_PADDING_VERTICAL
        >
            <Text string={label} font_size=FONT_BODY color={theme.text} align=TextAlign::Start />
        </Frame>
    }
}

#[component]
fn SelectPopup(children: Child) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame
            width=POPUP_WIDTH
            color={theme.surface_raised}
            outline={theme.border}
            outline_width=BORDER_WIDTH
            radius=RADIUS
            outline_visible=true
            padding_horizontal=POPUP_PADDING
            padding_vertical=POPUP_PADDING
        >
            {children}
        </Frame>
    }
}

pub fn select_selected(document: &Document, select: NodeId) -> Option<usize> {
    unstyled::select_selected(document, select)
}

pub fn select_open(document: &Document, select: NodeId) -> bool {
    unstyled::select_open(document, select)
}

fn option_background(theme: &ThemeStore, highlighted: bool, hovered: bool) -> Color32 {
    match (highlighted, hovered) {
        (true, _) => theme.accent_soft.get(),
        (false, true) => theme.surface.get(),
        (false, false) => Color32::TRANSPARENT,
    }
}

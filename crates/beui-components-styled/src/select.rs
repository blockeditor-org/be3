use crate::frame::{Border, ColorRole, Frame, Radius, focus_border};
use beui_core::font::Overflow;
use beui_macros::{component, view};

use crate::scroll::scrollbar_style;
use crate::sheet::AnchoredSheet;
use crate::text::{Icon, Text, TextColor};
use crate::text_input::text_input_style;
use crate::theme::field_border;
use crate::list::List;
use crate::tokens::{Length, Padding, Size, Space, Stroke, use_size, use_space};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{
    AnchoredStyle, ChoiceOption, SelectOptionHandle, SelectTriggerHandle, TextInputHandle,
};
use beui_core::base::TextAlign;
use beui_core::icons::ICON_CHEVRON_DOWN;
use beui_core::node::NodeId;
use beui_view::reactive::*;

const TRIGGER_MAX_WIDTH: f32 = 220.0;
const POPUP_WIDTH: Size = Size::Custom(220.0);
const POPUP_PADDING: Space = Space::Sm;
const OPTIONS_MAX_HEIGHT: Size = Size::Custom(240.0);
const HEIGHT: Size = Size::Field;
const PADDING_HORIZONTAL: Space = Space::Md;
const OPTION_PADDING_VERTICAL: Space = Space::Sm;
const ARROW_SPACING: Space = Space::Xs;

#[component]
pub fn Select(
    #[prop(default = ComponentRef::new())] api: ComponentRef<crate::SelectApi>,

    options: Children<ChoiceOption>,
    #[prop(default = None)] selected: Binding<Option<usize>>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = String::new())] placeholder: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = Length::Extent(Extent::at_most(TRIGGER_MAX_WIDTH)))] width: Prop<Length>,
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
            list_height={use_size(OPTIONS_MAX_HEIGHT).get_untracked()}
            spacing={use_space(POPUP_PADDING).get_untracked()}
            trigger={move |handle| view! {
                <SelectTrigger handle placeholder={placeholder} width={width} />
            }}
            option={|handle| view! {
                <SelectOption handle />
            }}
            surface={AnchoredStyle::new(
                |content| view! {
                    <SelectPopup>{content}</SelectPopup>
                },
                |handle| view! {
                    <AnchoredSheet handle padding=POPUP_PADDING />
                },
            )}
        />
    }
}

#[component]
fn SelectTrigger(
    handle: SelectTriggerHandle,
    placeholder: Memo<String>,
    width: Prop<Length>,
) -> NodeId {
    let SelectTriggerHandle {
        label,
        hovered,
        focused,
        disabled,
        ..
    } = handle;
    let empty = create_memo(move || label.with(Option::is_none));
    let label_text = create_memo(move || {
        label.get().unwrap_or_else(|| match placeholder.get() {
            placeholder if placeholder.is_empty() => "Select...".to_owned(),
            placeholder => placeholder,
        })
    });
    let border = create_memo(move || {
        Some(Border::new(field_border(
            disabled.get(),
            focused.get(),
            hovered.get(),
        )))
    });
    let fill = create_memo(move || match disabled.get() {
        true => ColorRole::Surface,
        false => ColorRole::SurfaceRaised,
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
            border={focus_border(focused, Border::FOCUS.offset(Stroke::Focus))}
            radius=Radius::Md
        >
            <Frame
                height=HEIGHT
                fill={fill}
                border={border}
                radius=Radius::Md
                padding={Padding::horizontal(PADDING_HORIZONTAL)}
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
    let border =
        create_memo(move || Some(Border::new(field_border(false, focused.get(), hovered.get()))));
    view! {
        <Frame
            height=HEIGHT
            fill=ColorRole::Surface
            border={border}
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
    let fill_color = create_memo(move || option_background(highlighted.get(), hovered.get()));
    view! {
        <Frame
            fill={fill_color}
            radius=Radius::Md
            padding={Padding::symmetric(PADDING_HORIZONTAL, OPTION_PADDING_VERTICAL)}
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

fn option_background(highlighted: bool, hovered: bool) -> ColorRole {
    match (highlighted, hovered) {
        (true, _) => ColorRole::AccentSoft,
        (false, true) => ColorRole::Surface,
        (false, false) => ColorRole::None,
    }
}

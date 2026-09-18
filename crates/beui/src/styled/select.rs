use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::color::Color32;
use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{Callback, Child, Frame, Prop, Text, clone, create_memo};
use crate::styled::context_menu::text_input_menu;
use crate::styled::theme::{BORDER_WIDTH, FONT_BODY, RADIUS, ThemeStore, use_theme};
use crate::unstyled;
use crate::unstyled::{SelectOptionHandle, SelectTriggerHandle, TextInputHandle};

const TRIGGER_WIDTH: f32 = 220.0;
const POPUP_WIDTH: f32 = 220.0;
const POPUP_PADDING: f32 = 6.0;
const HEIGHT: f32 = 34.0;
const PADDING_HORIZONTAL: f32 = 10.0;
const OPTION_PADDING_VERTICAL: f32 = 6.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 3.0;

#[component]
pub fn Select(
    options: Vec<String>,
    selected: Prop<Option<usize>>,
    #[prop(default = String::new())] label: Prop<String>,
    on_change: Callback<Option<usize>>,
) -> NodeId {
    let trigger_options = options.clone();
    let accessibility = label.map(|label| {
        let mut node = Node::new(Role::ComboBox);
        if !label.is_empty() {
            node.set_label(label);
        }
        node
    });
    let theme = use_theme();
    view! {
        <unstyled::Select
            options
            selected
            accessibility
            on_change={move |selected| on_change.call(selected)}
            search_placeholder="Search"
            search_font_size=FONT_BODY
            search_color={theme.text.clone()}
            search_placeholder_color={theme.text_muted.clone()}
            search_selection_color={theme.accent_soft.clone()}
            search_caret_color={theme.accent.clone()}
            search_padding_horizontal=PADDING_HORIZONTAL
            search_content={|handle| view! {
                <SearchField handle />
            }}
            search_menu={text_input_menu()}
            trigger={move |handle| view! {
                <SelectTrigger options={trigger_options} handle />
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
fn SelectTrigger(options: Vec<String>, handle: SelectTriggerHandle) -> NodeId {
    let SelectTriggerHandle {
        selected,
        hovered,
        focused,
        ..
    } = handle;
    let theme = use_theme();
    let label_text = create_memo(move || trigger_label(&options, selected.get()));
    let border = create_memo(
        clone!(focused theme -> move || border_color(&theme, focused.get(), hovered.get())),
    );
    view! {
        <Frame
            width=TRIGGER_WIDTH
            outline={theme.accent.clone()}
            outline_width=FOCUS_RING_WIDTH
            radius=RADIUS
            outline_offset=FOCUS_RING_OFFSET
            outline_visible={focused}
        >
            <Frame
                width=TRIGGER_WIDTH
                height=HEIGHT
                color={theme.surface_raised.clone()}
                outline={border}
                outline_width=BORDER_WIDTH
                radius=RADIUS
                outline_visible=true
                padding_horizontal=PADDING_HORIZONTAL
            >
                <Text
                    string={label_text}
                    font_size=FONT_BODY
                    color={theme.text.clone()}
                    align=TextAlign::Start
                    clip=true
                />
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
    } = handle;
    let theme = use_theme();
    let border =
        create_memo(clone!(theme -> move || border_color(&theme, focused.get(), hovered.get())));
    view! {
        <Frame
            height=HEIGHT
            color={theme.surface.clone()}
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
    let fill_color = create_memo(
        clone!(theme -> move || option_background(&theme, highlighted.get(), hovered.get())),
    );
    view! {
        <Frame
            color={fill_color}
            radius=RADIUS
            padding_horizontal=PADDING_HORIZONTAL
            padding_vertical=OPTION_PADDING_VERTICAL
        >
            <Text
                string={label}
                font_size=FONT_BODY
                color={theme.text.clone()}
                align=TextAlign::Start
            />
        </Frame>
    }
}

#[component]
fn SelectPopup(children: Child) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame
            width=POPUP_WIDTH
            color={theme.surface_raised.clone()}
            outline={theme.border.clone()}
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

fn trigger_label(options: &[String], selected: Option<usize>) -> String {
    selected
        .and_then(|index| options.get(index))
        .cloned()
        .unwrap_or_else(|| "Select...".to_owned())
}

fn option_background(theme: &ThemeStore, highlighted: bool, hovered: bool) -> Color32 {
    match (highlighted, hovered) {
        (true, _) => theme.accent_soft.get(),
        (false, true) => theme.surface.get(),
        (false, false) => Color32::TRANSPARENT,
    }
}

fn border_color(theme: &ThemeStore, focused: bool, hovered: bool) -> Color32 {
    match (focused, hovered) {
        (true, _) => theme.accent.get(),
        (false, true) => theme.text_muted.get(),
        (false, false) => theme.border.get(),
    }
}

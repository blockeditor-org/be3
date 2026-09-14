use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::color::Color32;
use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{create_memo, Callback, Child, Frame, Prop, Text};
use crate::styled::theme::{
    ACCENT, ACCENT_SOFT, BORDER, BORDER_WIDTH, FONT_BODY, RADIUS, SURFACE, SURFACE_RAISED, TEXT,
    TEXT_MUTED,
};
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
    view! {
        <unstyled::Select
            options
            selected
            accessibility
            on_change={move |selected| on_change.call(selected)}
            search_placeholder="Search"
            search_font_size=FONT_BODY
            search_color=TEXT
            search_placeholder_color=TEXT_MUTED
            search_selection_color=ACCENT_SOFT
            search_caret_color=ACCENT
            search_padding_horizontal=PADDING_HORIZONTAL
            search_content={|handle| view! { <SearchField handle /> }}
            trigger={move |handle| view! { <SelectTrigger options={trigger_options} handle /> }}
            option={|handle| view! { <SelectOption handle /> }}
        >
            {|content| view! { <SelectPopup>{content}</SelectPopup> }}
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
    let label_text = create_memo(move || trigger_label(&options, selected.get()));
    let border = create_memo({
        let focused = focused.clone();
        move || border_color(focused.get(), hovered.get())
    });
    view! {
        <Frame outline=ACCENT outline_width=FOCUS_RING_WIDTH radius=RADIUS outline_offset=FOCUS_RING_OFFSET outline_visible={focused}>
            <Frame
                width=TRIGGER_WIDTH
                height=HEIGHT
                color=SURFACE_RAISED
                outline={border}
                outline_width=BORDER_WIDTH
                radius=RADIUS
                outline_visible=true
                padding_horizontal=PADDING_HORIZONTAL
            >
                <Text
                    string={label_text}
                    font_size=FONT_BODY
                    color=TEXT
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
    let border = create_memo(move || border_color(focused.get(), hovered.get()));
    view! {
        <Frame height=HEIGHT color=SURFACE outline={border} outline_width=BORDER_WIDTH radius=RADIUS outline_visible=true>
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
        <Frame color={fill_color} radius=RADIUS padding_horizontal=PADDING_HORIZONTAL padding_vertical=OPTION_PADDING_VERTICAL>
            <Text
                string={label}
                font_size=FONT_BODY
                color=TEXT
                align=TextAlign::Start
            />
        </Frame>
    }
}

#[component]
fn SelectPopup(children: Child) -> NodeId {
    view! {
        <Frame
            width=POPUP_WIDTH
            color=SURFACE_RAISED
            outline=BORDER
            outline_width=BORDER_WIDTH
            radius=RADIUS
            outline_visible=true
            padding_horizontal=POPUP_PADDING
            padding_vertical=POPUP_PADDING
        >{children}</Frame>
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

fn option_background(highlighted: bool, hovered: bool) -> Color32 {
    match (highlighted, hovered) {
        (true, _) => ACCENT_SOFT,
        (false, true) => SURFACE,
        (false, false) => Color32::TRANSPARENT,
    }
}

fn border_color(focused: bool, hovered: bool) -> Color32 {
    match (focused, hovered) {
        (true, _) => ACCENT,
        (false, true) => TEXT_MUTED,
        (false, false) => BORDER,
    }
}

use beui_macros::{component, view};

use beui_core::color::Color32;

use crate::text::IconSized;
use crate::theme::{FOCUS_RING_WIDTH, FONT_HEADING, RADIUS, ThemeStore, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::DisclosureHandle;
use beui_core::base::TextAlign;
use beui_core::document::Document;
use beui_core::icons::{ICON_EXPAND_LESS, ICON_EXPAND_MORE};
use beui_core::node::NodeId;
use beui_view::reactive::{
    Align, Callback, Child, Direction, Frame, ItemSize, List, Memo, Prop, Text, clone, create_memo,
    focus_ring,
};

const SPACING: f32 = 8.0;
const PADDING_HORIZONTAL: f32 = 8.0;
const PADDING_VERTICAL: f32 = 6.0;

#[component]
pub fn Accordion(
    title: Prop<String>,
    open: Prop<bool>,
    on_toggle: Callback<bool>,
    children: Child,
) -> NodeId {
    let title_text = create_memo(move || title.get());
    view! {
        <unstyled::Disclosure
            spacing=SPACING
            on_toggle={move |open| on_toggle.call(open)}
            header={move |handle| view! {
                <AccordionHeader handle title={title_text} />
            }}
            open
        >
            <Frame padding_horizontal=PADDING_HORIZONTAL>{children}</Frame>
        </unstyled::Disclosure>
    }
}

#[component]
fn AccordionHeader(handle: DisclosureHandle, title: Memo<String>) -> NodeId {
    let DisclosureHandle {
        hovered,
        open,
        focused,
        ..
    } = handle;
    let theme = use_theme();
    let header_color = create_memo(clone!(theme -> move || header_fill(&theme, hovered.get())));
    let marker_glyph = create_memo(move || glyph(open.get()).to_owned());
    let marker_color = theme.text_muted.clone();
    let title_color = theme.text.clone();
    view! {
        <Frame
            color={header_color}
            outline={theme.accent.clone()}
            outline_width=FOCUS_RING_WIDTH
            radius=RADIUS
            outline_offset=2.0
            outline_visible={focus_ring(focused)}
            padding_horizontal=PADDING_HORIZONTAL
            padding_vertical=PADDING_VERTICAL
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Text
                    @sizing=ItemSize::Percent(100.0)
                    string={title}
                    font_size=FONT_HEADING
                    color={title_color}
                    align=TextAlign::Start
                />
                <IconSized glyph={marker_glyph} font_size=FONT_HEADING color={marker_color} />
            </List>
        </Frame>
    }
}

pub fn accordion_open(document: &Document, accordion: NodeId) -> bool {
    unstyled::disclosure_open(document, accordion)
}

fn glyph(open: bool) -> &'static str {
    if open {
        ICON_EXPAND_LESS
    } else {
        ICON_EXPAND_MORE
    }
}

fn header_fill(theme: &ThemeStore, hovered: bool) -> Color32 {
    if hovered {
        theme.hover.get()
    } else {
        Color32::TRANSPARENT
    }
}

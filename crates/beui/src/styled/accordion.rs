use beui_macros::{component, view};

use crate::color::Color32;

use crate::base::TextAlign;
use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    Align, Callback, Child, Direction, Frame, ItemSize, List, Memo, Prop, Text, clone, create_memo,
};
use crate::styled::theme::{FONT_HEADING, FONT_SMALL, RADIUS, ThemeStore, use_theme};
use crate::unstyled;
use crate::unstyled::DisclosureHandle;

const SPACING: f32 = 10.0;
const MARKER_WIDTH: f32 = 12.0;
const PADDING_HORIZONTAL: f32 = 6.0;
const PADDING_VERTICAL: f32 = 4.0;

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
            {children}
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
            outline_width=2.0
            radius=RADIUS
            outline_offset=2.0
            outline_visible={focused}
            padding_horizontal=PADDING_HORIZONTAL
            padding_vertical=PADDING_VERTICAL
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Frame width=MARKER_WIDTH>
                    <Text
                        string={marker_glyph}
                        font_size=FONT_SMALL
                        color={marker_color}
                        monospace=true
                        align=TextAlign::Center
                    />
                </Frame>
                <Text
                    @sizing=ItemSize::Percent(100.0)
                    string={title}
                    font_size=FONT_HEADING
                    color={title_color}
                    align=TextAlign::Start
                />
            </List>
        </Frame>
    }
}

pub fn accordion_open(document: &Document, accordion: NodeId) -> bool {
    unstyled::disclosure_open(document, accordion)
}

fn glyph(open: bool) -> &'static str {
    if open { "-" } else { "+" }
}

fn header_fill(theme: &ThemeStore, hovered: bool) -> Color32 {
    if hovered {
        theme.hover.get()
    } else {
        Color32::TRANSPARENT
    }
}

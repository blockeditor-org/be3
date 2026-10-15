use crate::frame::{Border, ColorRole, Frame, Radius, focus_border};
use beui_macros::{component, view};

use beui_core::color::Color32;

use crate::text::{Icon, Text, TextColor, TextSize};
use crate::theme::{ThemeStore, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::DisclosureHandle;
use beui_core::base::TextAlign;
use beui_core::document::Document;
use beui_core::icons::{ICON_EXPAND_LESS, ICON_EXPAND_MORE};
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

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
            on_toggle={on_toggle}
            header={move |handle| view! {
                <AccordionHeader handle title={title_text} />
            }}
            open
        >
            <Frame padding={Sides::horizontal(PADDING_HORIZONTAL)}>{children}</Frame>
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
    let header_color = create_memo(move || header_fill(&theme, hovered.get()));
    let marker_glyph = create_memo(move || glyph(open.get()).to_owned());
    let marker_color = TextColor::Muted;
    let title_color = TextColor::Default;
    view! {
        <Frame
            fill={ColorRole::custom(header_color)}
            border={focus_border(focused, Border::FOCUS.offset(2.0))}
            radius=Radius::Md
            padding={Sides::symmetric(PADDING_HORIZONTAL, PADDING_VERTICAL)}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Text
                    @sizing=ItemSize::Percent(100.0)
                    string={title}
                    size=TextSize::Lg
                    color={title_color}
                    align=TextAlign::Start
                />
                <Text size=TextSize::Sm color={marker_color} align=TextAlign::Center>
                    <Icon glyph={marker_glyph} />
                </Text>
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

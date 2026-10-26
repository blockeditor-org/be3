use crate::frame::{Border, ColorRole, Frame, Radius, focus_border};
use beui_view::types::icons::Glyph;
use beui_macros::{component, view};

use crate::list::List;
use crate::text::{Icon, Text, TextColor, TextSize};
use crate::tokens::{Padding, Space, Stroke, use_space};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::DisclosureHandle;
use beui_view::types::TextAlign;
use beui_view::types::icons::{ICON_CHEVRON_DOWN, ICON_CHEVRON_UP};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;

const SPACING: Space = Space::Md;
const PADDING_HORIZONTAL: Space = Space::Md;
const PADDING_VERTICAL: Space = Space::Sm;

#[component]
pub fn Accordion(
    #[prop(default = ComponentRef::new())] api: ComponentRef<crate::DisclosureApi>,
    title: Prop<String>,
    open: Binding<bool>,
    children: Child,
) -> NodeId {
    let title_text = create_memo(move || title.get());
    view! {
        <unstyled::Disclosure
            api
            spacing={use_space(SPACING).get_untracked()}
            header={move |handle| view! {
                <AccordionHeader handle title={title_text} />
            }}
            open
        >
            <Frame padding={Padding::horizontal(PADDING_HORIZONTAL)}>{children}</Frame>
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
    let header_color = create_memo(move || header_fill(hovered.get()));
    let marker_glyph = create_memo(move || glyph(open.get()).to_owned());
    let marker_color = TextColor::Muted;
    let title_color = TextColor::Default;
    view! {
        <Frame
            fill={header_color}
            border={focus_border(focused, Border::FOCUS.offset(Stroke::Focus))}
            radius=Radius::Md
            padding={Padding::symmetric(PADDING_HORIZONTAL, PADDING_VERTICAL)}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Text
                    @sizing=ItemSize::Percent(100.0)
                    size=TextSize::Lg
                    color={title_color}
                    align=TextAlign::Start
                >
                    {title}
                </Text>
                <Text size=TextSize::Sm color={marker_color} align=TextAlign::Center>
                    <Icon glyph={marker_glyph} />
                </Text>
            </List>
        </Frame>
    }
}

fn glyph(open: bool) -> Glyph {
    if open {
        ICON_CHEVRON_UP
    } else {
        ICON_CHEVRON_DOWN
    }
}

fn header_fill(hovered: bool) -> ColorRole {
    if hovered {
        ColorRole::Hover
    } else {
        ColorRole::None
    }
}

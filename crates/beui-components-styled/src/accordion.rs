use crate::frame::{ColorRole, Frame, Radius};
use beui_macros::{component, view};
use beui_view::types::icons::Glyph;

use crate::list::List;
use crate::text::{Icon, Text, TextColor, TextSize};
use crate::tokens::{Padding, Space, Stroke};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::DisclosureHandle;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::icons::{ICON_CHEVRON_DOWN, ICON_CHEVRON_UP};
use beui_view::types::{Align, Direction};
use beui_view::types::{Overflow, TextAlign};

const SPACING: Space = Space::Md;
const PADDING_HORIZONTAL: Space = Space::Md;
const PADDING_VERTICAL: Space = Space::Sm;

/// A header that shows or hides what it holds. `size` is the title's, smaller
/// for options inside a form; `detail`, muted after the title, says what the
/// closed content holds.
#[component]
pub fn Accordion(
    #[prop(default = ComponentRef::new())] api: ComponentRef<crate::DisclosureApi>,
    title: Prop<String>,
    #[prop(default = String::new())] detail: Prop<String>,
    #[prop(default = TextSize::Lg)] size: TextSize,
    open: Binding<bool>,
    children: Child,
) -> NodeId {
    let title_text = create_memo(move || title.get());
    let detail = create_memo(move || detail.get());
    view! {
        <unstyled::Disclosure
            api
            spacing={SPACING.px()}
            header={move |handle| view! {
                <AccordionHeader handle title={title_text} detail size />
            }}
            open
        >
            <Frame padding={Padding::horizontal(PADDING_HORIZONTAL)}>{children}</Frame>
        </unstyled::Disclosure>
    }
}

#[component]
fn AccordionHeader(
    handle: DisclosureHandle,
    title: Memo<String>,
    detail: Memo<String>,
    size: TextSize,
) -> NodeId {
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
    let detailed = create_memo(move || !detail.get().is_empty());
    view! {
        <Frame
            fill={header_color}
            focus={focused}
            focus_offset=Stroke::Focus
            radius=Radius::Md
            padding={Padding::symmetric(PADDING_HORIZONTAL, PADDING_VERTICAL)}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <ListItem grow=1.0>
                    <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                        <Text size={size} color={title_color} align=TextAlign::Start>{title}</Text>
                        <Show condition={detailed}>
                            <ListItem grow=1.0>
                                <Text
                                    overflow=Overflow::Ellipsis
                                    size=TextSize::Sm
                                    color=TextColor::Muted
                                    align=TextAlign::Start
                                >
                                    {detail}
                                </Text>
                            </ListItem>
                        </Show>
                    </List>
                </ListItem>
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

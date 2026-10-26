use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_macros::{component, view};

use crate::text::{Text, TextColor, TextSize};
use beui_components_unstyled::{CompletionMenu, CompletionRowHandle};
use beui_core::base::{Align, Direction};
use beui_core::font::TextAlign;
use beui_core::node::NodeId;
use crate::list::List;
use crate::tokens::{Padding, Size, Space, Stroke};
use beui_view::reactive::*;

const MENU_WIDTH: Size = Size::Custom(240.0);
const EMOJI_COLUMN: Size = Size::Md;

pub fn emoji_menu() -> CompletionMenu {
    CompletionMenu::new(
        |handle| {
            view! {
                <EmojiRow handle />
            }
        },
        |content| {
            view! {
                <EmojiPanel>{content}</EmojiPanel>
            }
        },
    )
}

#[component]
fn EmojiPanel(children: Child) -> NodeId {
    view! {
        <Frame
            width=MENU_WIDTH
            fill=ColorRole::SurfaceRaised
            border={Border::new(ColorRole::Border).width(Stroke::Hairline)}
            radius=Radius::Md
            padding=Space::Xs
        >
            {children}
        </Frame>
    }
}

#[component]
fn EmojiRow(handle: CompletionRowHandle) -> NodeId {
    let CompletionRowHandle {
        completion,
        highlighted,
        ..
    } = handle;
    let emoji = create_memo(move || completion.get().map(|item| item.insert).unwrap_or_default());
    let label = create_memo(move || completion.get().map(|item| item.label).unwrap_or_default());
    let fill = create_memo(move || match highlighted.get() {
        true => ColorRole::AccentSoft,
        false => ColorRole::None,
    });
    view! {
        <Frame
            fill={fill}
            radius=Radius::Md
            padding={Padding::symmetric(Space::Md, Space::Xs2)}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Frame width=EMOJI_COLUMN>
                    <Text size=TextSize::Lg align=TextAlign::Center>{emoji}</Text>
                </Frame>
                <Text size=TextSize::Sm color=TextColor::Muted>{label}</Text>
            </List>
        </Frame>
    }
}

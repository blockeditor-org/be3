use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_macros::{component, view};

use crate::text::{Text, TextColor, TextSize};
use crate::theme::use_theme;
use beui_components_unstyled::{CompletionMenu, CompletionRowHandle};
use beui_core::base::{Align, Direction};
use beui_core::color::Color32;
use beui_core::font::TextAlign;
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const MENU_WIDTH: f32 = 240.0;
const EMOJI_COLUMN: f32 = 22.0;

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
            border={Border::new(ColorRole::Border).width(1.0)}
            radius=Radius::Md
            padding=4.0
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
    let theme = use_theme();
    let emoji = create_memo(move || completion.get().map(|item| item.insert).unwrap_or_default());
    let label = create_memo(move || completion.get().map(|item| item.label).unwrap_or_default());
    let fill = create_memo(move || match highlighted.get() {
        true => theme.accent_soft.get(),
        false => Color32::TRANSPARENT,
    });
    view! {
        <Frame
            fill={ColorRole::custom(fill)}
            radius=Radius::Md
            padding={Sides::symmetric(8.0, 3.0)}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Frame width=EMOJI_COLUMN>
                    <Text size=TextSize::Lg align=TextAlign::Center>{emoji}</Text>
                </Frame>
                <Text size=TextSize::Sm color=TextColor::Muted>{label}</Text>
            </List>
        </Frame>
    }
}

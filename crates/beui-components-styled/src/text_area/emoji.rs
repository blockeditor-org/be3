use beui_macros::{component, view};

use crate::theme::{RADIUS, use_theme};
use beui_components_unstyled::{CompletionMenu, CompletionRowHandle};
use beui_core::base::{Align, Direction};
use beui_core::color::Color32;
use beui_core::font::TextAlign;
use beui_core::node::NodeId;
use beui_view::reactive::{Child, Frame, List, Text, clone, create_memo};

const MENU_WIDTH: f32 = 240.0;
const EMOJI_SIZE: f32 = 16.0;
const LABEL_SIZE: f32 = 12.0;
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
    let theme = use_theme();
    view! {
        <Frame
            width=MENU_WIDTH
            color={theme.surface_raised.clone()}
            outline={theme.border.clone()}
            outline_width=1.0
            outline_visible=true
            radius=RADIUS
            padding_horizontal=4.0
            padding_vertical=4.0
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
    let emoji = create_memo(clone!(completion -> move || {
        completion.get().map(|item| item.insert).unwrap_or_default()
    }));
    let label = create_memo(move || completion.get().map(|item| item.label).unwrap_or_default());
    let fill = create_memo(clone!(theme -> move || match highlighted.get() {
        true => theme.accent_soft.get(),
        false => Color32::TRANSPARENT,
    }));
    view! {
        <Frame color={fill} radius=RADIUS padding_horizontal=8.0 padding_vertical=3.0>
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Frame width=EMOJI_COLUMN>
                    <Text
                        string={emoji}
                        font_size=EMOJI_SIZE
                        color={theme.text.clone()}
                        align=TextAlign::Center
                    />
                </Frame>
                <Text string={label} font_size=LABEL_SIZE color={theme.text_muted.clone()} />
            </List>
        </Frame>
    }
}

use beui_macros::{component, view};

use crate::theme::{RADIUS, use_theme};
use beui_components_unstyled::{Completer, Completion, CompletionMenu};
use beui_core::base::{Align, Direction};
use beui_core::color::Color32;
use beui_core::font::TextAlign;
use beui_core::input::CursorIcon;
use beui_core::node::NodeId;
use beui_view::reactive::{ClickCatcher, ForEach, Frame, List, Text, clone, create_memo};

const RESULTS: usize = 8;
const MENU_WIDTH: f32 = 240.0;
const EMOJI_SIZE: f32 = 16.0;
const LABEL_SIZE: f32 = 12.0;
const EMOJI_COLUMN: f32 = 22.0;
const POPULAR: [&str; RESULTS] = [
    "smile", "joy", "heart", "+1", "tada", "fire", "eyes", "rocket",
];

pub fn emoji_completer() -> Completer {
    Completer::new(':', search_emoji)
}

pub fn search_emoji(query: &str) -> Vec<Completion> {
    let query = query.to_ascii_lowercase();
    if query.is_empty() {
        return POPULAR
            .iter()
            .filter_map(|code| Some(completion(emojis::get_by_shortcode(code)?, code)))
            .collect();
    }
    let mut ranked: Vec<(u8, usize, usize, Completion)> = Vec::new();
    for (order, emoji) in emojis::iter().filter(|emoji| drawable(emoji)).enumerate() {
        let best = emoji
            .shortcodes()
            .filter_map(|code| rank(code, &query).map(|rank| (rank, code)))
            .min_by_key(|(rank, code)| (*rank, code.len()));
        if let Some((rank, code)) = best {
            ranked.push((rank, code.len(), order, completion(emoji, code)));
        }
    }
    ranked.sort_by_key(|(rank, length, order, _)| (*rank, *length, *order));
    ranked
        .into_iter()
        .take(RESULTS)
        .map(|(_, _, _, completion)| completion)
        .collect()
}

fn drawable(emoji: &emojis::Emoji) -> bool {
    !emoji
        .as_str()
        .chars()
        .any(|character| matches!(character, '\u{1f1e6}'..='\u{1f1ff}' | '\u{e0020}'..='\u{e007f}'))
}

fn rank(code: &str, query: &str) -> Option<u8> {
    if code == query {
        Some(0)
    } else if code.starts_with(query) {
        Some(1)
    } else if code
        .split(['_', '-'])
        .skip(1)
        .any(|word| word.starts_with(query))
    {
        Some(2)
    } else if code.contains(query) {
        Some(3)
    } else {
        None
    }
}

fn completion(emoji: &emojis::Emoji, code: &str) -> Completion {
    Completion {
        label: format!(":{code}:"),
        insert: emoji.as_str().to_owned(),
    }
}

#[component]
pub fn EmojiMenu(menu: CompletionMenu) -> NodeId {
    let theme = use_theme();
    let items = menu.items.clone();
    let keys = create_memo(clone!(items -> move || (0..items.get().len()).collect::<Vec<usize>>()));
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
            <List spacing=0.0>
                <ForEach keys>
                    {move |index: usize| {
                        let menu = menu.clone();
                        view! { <EmojiRow menu index /> }
                    }}
                </ForEach>
            </List>
        </Frame>
    }
}

#[component]
fn EmojiRow(menu: CompletionMenu, index: usize) -> NodeId {
    let theme = use_theme();
    let CompletionMenu {
        items,
        highlighted,
        pick,
        highlight,
    } = menu;
    let item = create_memo(clone!(items -> move || items.get().get(index).cloned()));
    let emoji = create_memo(clone!(item -> move || item.get().map(|item| item.insert).unwrap_or_default()));
    let label = create_memo(clone!(item -> move || item.get().map(|item| item.label).unwrap_or_default()));
    let fill = create_memo(clone!(theme -> move || match highlighted.get() == index {
        true => theme.accent_soft.get(),
        false => Color32::TRANSPARENT,
    }));
    view! {
        <ClickCatcher
            @test_id={format!("text.emoji.{index}")}
            cursor=CursorIcon::PointingHand
            on_click={move || pick.call(index)}
            on_hover_change={move |hovered: bool| {
                if hovered {
                    highlight.call(index);
                }
            }}
        >
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
        </ClickCatcher>
    }
}

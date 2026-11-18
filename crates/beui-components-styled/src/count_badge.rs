use beui_macros::{component, view};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::TextAlign;

use crate::frame::{ColorRole, Frame, Radius};
use crate::text::{Text, TextColor, TextSize, Weight};
use crate::tokens::{Padding, Space};

const PADDING: Padding = Padding::symmetric(Space::Xs, Space::None);

/// The most a badge spells out; more shows as this and a plus.
pub const COUNT_BADGE_MOST: u32 = 99;

/// What a badge says for `count`: the number, or `COUNT_BADGE_MOST` and a plus past it.
pub fn count_badge_text(count: u32) -> String {
    match count > COUNT_BADGE_MOST {
        true => format!("{COUNT_BADGE_MOST}+"),
        false => count.to_string(),
    }
}

/// A count on something it belongs to, as a messaging app marks a conversation's new messages:
/// a small accent pill holding the number. Nothing shows at zero.
#[component]
pub fn CountBadge(count: Prop<u32>) -> NodeId {
    let count = create_memo(move || count.get());
    let text = create_memo(move || count_badge_text(count.get()));
    view! {
        <Show value={move || count.get() > 0}>
            <Frame fill=ColorRole::Accent radius=Radius::Full padding=PADDING>
                <Text
                    size=TextSize::Xs
                    weight=Weight::Bold
                    color=TextColor::OnAccent
                    align=TextAlign::Center
                >
                    {text}
                </Text>
            </Frame>
        </Show>
    }
}

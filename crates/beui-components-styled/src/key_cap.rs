use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_macros::{component, view};
use beui_view::types::Overflow;

use crate::text::{Text, TextColor, TextSize, Weight};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::TextAlign;

use crate::tokens::{Padding, Space};

const PADDING: Padding = Padding::symmetric(Space::Sm, Space::Xs2);

/// A key, button or wheel turn drawn as a key on a keyboard, for showing what
/// is pressed; `Chip` is for tags. `muted` draws one that only stands in for a
/// key, such as a wait between taps.
#[component]
pub fn KeyCap(label: Prop<String>, #[prop(default = false)] muted: Prop<bool>) -> NodeId {
    let label_text = create_memo(move || label.get());
    let muted = create_memo(move || muted.get());
    let fill = create_memo(move || match muted.get() {
        true => ColorRole::None,
        false => ColorRole::SurfaceRaised,
    });
    let border = create_memo(move || {
        Some(Border::new(match muted.get() {
            true => ColorRole::Border,
            false => ColorRole::ControlOutline,
        }))
    });
    let color = create_memo(move || match muted.get() {
        true => TextColor::Muted,
        false => TextColor::Default,
    });
    view! {
        <Frame fill={fill} border={border} radius=Radius::Sm padding=PADDING>
            <Text
                overflow=Overflow::Ellipsis
                size=TextSize::Sm
                weight=Weight::Bold
                color={color}
                align=TextAlign::Center
            >
                {label_text}
            </Text>
        </Frame>
    }
}

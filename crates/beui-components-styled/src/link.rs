use crate::frame::Radius;
use accesskit::Role;
use beui_macros::{component, view};
use beui_view::types::icons::Glyph;

use crate::focus_ring::FocusRing;
use crate::text::{Icon, Text, TextColor, TextSize};
use crate::tokens::Stroke;
use beui_components_unstyled as unstyled;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;

#[component]
pub fn Link(
    label: Prop<String>,
    #[prop(default = None)] glyph: Prop<Option<Glyph>>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = TextSize::Base)] size: Prop<TextSize>,
    on_click: ClickCallback,
) -> NodeId {
    view! {
        <unstyled::Button label glyph role=Role::Link disabled on_click={on_click}>|handle|
            <LinkFace handle size />
        </unstyled::Button>
    }
}

#[component]
fn LinkFace(handle: unstyled::ButtonHandle, size: Prop<TextSize>) -> NodeId {
    let unstyled::ButtonHandle {
        hovered,
        active,
        focused,
        disabled,
        label,
        glyph,
        ..
    } = handle;
    let color = create_memo(move || text_color(disabled.get(), hovered.get(), active.get()));
    let underlined = create_memo(move || hovered.get() || focused.get());
    view! {
        <FocusRing focused radius=Radius::Xs offset=Stroke::Focus>
            <Text size color underline={underlined}>
                <Icon glyph={glyph} />
                {label}
            </Text>
        </FocusRing>
    }
}

fn text_color(disabled: bool, hovered: bool, active: bool) -> TextColor {
    if disabled {
        return TextColor::Muted;
    }
    match (hovered, active) {
        (_, true) => TextColor::AccentActive,
        (true, false) => TextColor::AccentHover,
        (false, false) => TextColor::Accent,
    }
}

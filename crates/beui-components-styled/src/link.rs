use crate::frame::Radius;
use accesskit::Role;
use beui_macros::{component, view};

use crate::focus_ring::FocusRing;
use crate::text::{Icon, Text, TextColor, TextSize};
use crate::theme::{ThemeStore, use_theme};
use beui_components_unstyled as unstyled;
use beui_core::node::NodeId;
use beui_view::reactive::*;

const FOCUS_RING_OFFSET: f32 = 3.0;
const FOCUS_RING_RADIUS: f32 = 4.0;

#[component]
pub fn Link(
    label: Prop<String>,
    #[prop(default = String::new())] glyph: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = TextSize::Base)] size: Prop<TextSize>,
    on_click: ClickCallback,
) -> NodeId {
    view! {
        <unstyled::Button
            label
            glyph
            role=Role::Link
            disabled
            on_click={on_click}
            content={move |handle| view! {
                <LinkFace handle size />
            }}
        />
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
    let theme = use_theme();
    let color =
        create_memo(move || text_color(&theme, disabled.get(), hovered.get(), active.get()));
    let underlined = create_memo(move || hovered.get() || focused.get());
    view! {
        <FocusRing focused radius={Radius::custom(FOCUS_RING_RADIUS)} offset=FOCUS_RING_OFFSET>
            <Text size color underline={underlined}>
                <Icon glyph={glyph} />
                {label}
            </Text>
        </FocusRing>
    }
}

fn text_color(theme: &ThemeStore, disabled: bool, hovered: bool, active: bool) -> TextColor {
    if disabled {
        return TextColor::Muted;
    }
    match (hovered, active) {
        (_, true) => TextColor::Custom(theme.accent_active.get()),
        (true, false) => TextColor::Custom(theme.accent_hover.get()),
        (false, false) => TextColor::Accent,
    }
}

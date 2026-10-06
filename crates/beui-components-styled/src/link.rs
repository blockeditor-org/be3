use accesskit::Role;
use beui_macros::{component, view};

use crate::focus_ring::FocusRing;
use crate::text::Icon;
use crate::theme::{FONT_BODY, ThemeStore, use_theme};
use beui_components_unstyled as unstyled;
use beui_core::base::TextAlign;
use beui_core::color::Color32;
use beui_core::node::NodeId;
use beui_view::reactive::{
    Align, ClickCallback, Direction, List, Prop, Show, Text, clone, create_memo,
};

const ICON_SPACING: f32 = 6.0;
const FOCUS_RING_OFFSET: f32 = 3.0;
const FOCUS_RING_RADIUS: u8 = 4;

#[component]
pub fn Link(
    label: Prop<String>,
    #[prop(default = String::new())] glyph: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = FONT_BODY)] font_size: Prop<f32>,
    on_click: ClickCallback,
) -> NodeId {
    view! {
        <unstyled::Button
            label
            glyph
            role=Role::Link
            disabled
            on_click={move || on_click.call()}
            content={move |handle| view! {
                <LinkFace handle font_size />
            }}
        />
    }
}

#[component]
fn LinkFace(handle: unstyled::ButtonHandle, font_size: Prop<f32>) -> NodeId {
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
    let color = create_memo(clone!(theme hovered active -> move || {
        text_color(&theme, disabled.get(), hovered.get(), active.get())
    }));
    let underlined = create_memo(clone!(hovered focused -> move || hovered.get() || focused.get()));
    let has_glyph = create_memo(clone!(glyph -> move || !glyph.get().is_empty()));
    let icon_color = color.clone();
    let label_color = color;
    let size = create_memo(move || font_size.get());
    let icon_size = size.clone();
    view! {
        <FocusRing focused radius=FOCUS_RING_RADIUS offset=FOCUS_RING_OFFSET>
            <List direction=Direction::Horizontal align=Align::Center spacing=ICON_SPACING>
                <Show condition={has_glyph}>
                    <Icon
                        glyph={glyph.clone()}
                        text_size={icon_size.clone()}
                        color={icon_color.clone()}
                    />
                </Show>
                <Text
                    string={label}
                    font_size={size}
                    color={label_color}
                    align=TextAlign::Start
                    underline={underlined}
                />
            </List>
        </FocusRing>
    }
}

fn text_color(theme: &ThemeStore, disabled: bool, hovered: bool, active: bool) -> Color32 {
    if disabled {
        return theme.text_muted.get();
    }
    match (hovered, active) {
        (_, true) => theme.accent_active.get(),
        (true, false) => theme.accent_hover.get(),
        (false, false) => theme.accent.get(),
    }
}

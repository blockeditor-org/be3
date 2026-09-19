use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::color::Color32;
use crate::node::NodeId;
use crate::reactive::{CenteredRow, ClickCallback, Frame, Prop, Show, Text, clone, create_memo};
use crate::styled::text::IconSized;
use crate::styled::theme::{FONT_BODY, ICON_SIZE, ThemeStore, use_theme};
use crate::unstyled;

const ICON_SPACING: f32 = 6.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
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
    let text = create_memo(move || label.get());
    let disabled = create_memo(move || disabled.get());
    let accessibility = create_memo(clone!(text -> move || {
        let mut node = Node::new(Role::Link);
        node.set_label(text.get());
        node
    }));
    view! {
        <unstyled::Button
            disabled={disabled.clone()}
            accessibility
            on_click={move || on_click.call()}
            content={move |handle| view! {
                <LinkFace handle label={text.clone()} glyph disabled font_size />
            }}
        />
    }
}

#[component]
fn LinkFace(
    handle: unstyled::ButtonHandle,
    label: Prop<String>,
    glyph: Prop<String>,
    disabled: Prop<bool>,
    font_size: Prop<f32>,
) -> NodeId {
    let unstyled::ButtonHandle {
        hovered,
        active,
        focused,
    } = handle;
    let theme = use_theme();
    let color = create_memo(clone!(theme hovered active -> move || {
        text_color(&theme, disabled.get(), hovered.get(), active.get())
    }));
    let underlined = create_memo(clone!(hovered focused -> move || hovered.get() || focused.get()));
    let glyph_text = create_memo(move || glyph.get());
    let has_glyph = create_memo(clone!(glyph_text -> move || !glyph_text.get().is_empty()));
    let icon_color = color.clone();
    let label_color = color;
    let size = create_memo(move || font_size.get());
    let icon_size = create_memo(clone!(size -> move || size.get() * ICON_SIZE / FONT_BODY));
    view! {
        <Frame
            outline={theme.accent.clone()}
            outline_width=FOCUS_RING_WIDTH
            radius=FOCUS_RING_RADIUS
            outline_offset=FOCUS_RING_OFFSET
            outline_visible={focused}
        >
            <CenteredRow spacing=ICON_SPACING>
                <Show condition={has_glyph}>
                    <IconSized glyph={glyph_text} font_size={icon_size} color={icon_color} />
                </Show>
                <Text
                    string={label}
                    font_size={size}
                    color={label_color}
                    align=TextAlign::Start
                    underline={underlined}
                />
            </CenteredRow>
        </Frame>
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

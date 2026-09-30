use beui_macros::{component, view};

use crate::text::Icon;
use crate::theme::{FONT_BODY, FONT_SMALL, RADIUS, use_theme};
use beui_components_unstyled as unstyled;
use beui_core::node::NodeId;
use beui_view::reactive::{
    Align, ClickCallback, Direction, Frame, ItemSize, List, Prop, Show, Text, clone, create_memo,
    focus_ring,
};

const ROW_HEIGHT: f32 = 52.0;
const PADDING_HORIZONTAL: f32 = 12.0;
const SPACING: f32 = 14.0;

#[component]
pub fn ActionRow(
    label: Prop<String>,
    #[prop(default = String::new())] glyph: Prop<String>,
    #[prop(default = String::new())] detail: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] danger: bool,
    on_click: ClickCallback,
) -> NodeId {
    let disabled = create_memo(move || disabled.get());
    let face = disabled.clone();
    view! {
        <unstyled::Button
            disabled
            on_click={move || on_click.call()}
            content={move |handle| view! {
                <ActionRowFace handle label glyph detail disabled={face.clone()} danger />
            }}
        />
    }
}

#[component]
fn ActionRowFace(
    handle: unstyled::ButtonHandle,
    label: Prop<String>,
    glyph: Prop<String>,
    detail: Prop<String>,
    disabled: Prop<bool>,
    danger: bool,
) -> NodeId {
    let unstyled::ButtonHandle {
        hovered,
        active,
        focused,
    } = handle;
    let theme = use_theme();
    let off = create_memo(move || disabled.get());
    let fill = create_memo(clone!(theme off -> move || {
        match (off.get(), active.get(), hovered.get()) {
            (true, _, _) => beui_core::color::Color32::TRANSPARENT,
            (false, true, _) => theme.pressed.get(),
            (false, false, true) => theme.hover.get(),
            (false, false, false) => beui_core::color::Color32::TRANSPARENT,
        }
    }));
    let text_color = create_memo(clone!(theme off -> move || {
        match (off.get(), danger) {
            (true, _) => theme.text_muted.get(),
            (false, true) => theme.danger.get(),
            (false, false) => theme.text.get(),
        }
    }));
    let icon_color = create_memo(clone!(theme off -> move || {
        match (off.get(), danger) {
            (false, true) => theme.danger.get(),
            _ => theme.text_muted.get(),
        }
    }));
    let glyph = create_memo(move || glyph.get());
    let has_glyph = create_memo(clone!(glyph -> move || !glyph.get().is_empty()));
    let detail = create_memo(move || detail.get());
    let has_detail = create_memo(clone!(detail -> move || !detail.get().is_empty()));
    view! {
        <Frame
            height=ROW_HEIGHT
            color={fill}
            radius=RADIUS
            outline={theme.accent.clone()}
            outline_width=2.0
            outline_visible={focus_ring(focused)}
            padding_horizontal=PADDING_HORIZONTAL
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Show condition={has_glyph}>
                    <Icon glyph={glyph.clone()} color={icon_color.clone()} />
                </Show>
                <List @sizing=ItemSize::Percent(100.0) spacing=2.0>
                    <Text string={label} font_size=FONT_BODY color={text_color} ellipsis=true />
                    <Show condition={has_detail}>
                        <Text
                            string={detail.clone()}
                            font_size=FONT_SMALL
                            color={theme.text_muted.clone()}
                            ellipsis=true
                        />
                    </Show>
                </List>
            </List>
        </Frame>
    }
}

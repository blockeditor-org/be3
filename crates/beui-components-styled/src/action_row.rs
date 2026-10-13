use beui_macros::{component, view};

use crate::text::Icon;
use crate::theme::{FOCUS_RING_WIDTH, FONT_BODY, FONT_SMALL, RADIUS, use_theme};
use beui_components_unstyled as unstyled;
use beui_core::icons::ICON_CHEVRON_RIGHT;
use beui_core::node::NodeId;
use beui_view::reactive::{
    Action, Align, ClickCallback, Direction, Frame, ItemSize, List, Prop, ReadSignal, Show, Text,
    clone, create_memo, focus_ring,
};

const ROW_HEIGHT: f32 = 52.0;
const PADDING_HORIZONTAL: f32 = 12.0;
const SPACING: f32 = 14.0;

#[component]
pub fn ActionRow(
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = String::new())] glyph: Prop<String>,
    #[prop(default = String::new())] detail: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] danger: bool,
    action: Option<Action>,
    on_click: ClickCallback,
) -> NodeId {
    view! {
        <unstyled::Button
            label
            glyph
            action
            disabled
            on_click={move || on_click.call()}
            content={move |handle: unstyled::ButtonHandle| {
                let unstyled::ButtonHandle {
                    hovered,
                    active,
                    focused,
                    disabled,
                    label,
                    glyph,
                    ..
                } = handle;
                view! {
                    <ActionRowFace hovered active focused label glyph detail disabled danger />
                }
            }}
        />
    }
}

#[component]
pub(crate) fn ActionRowFace(
    hovered: ReadSignal<bool>,
    active: ReadSignal<bool>,
    focused: ReadSignal<bool>,
    label: Prop<String>,
    glyph: Prop<String>,
    detail: Prop<String>,
    disabled: Prop<bool>,
    danger: Prop<bool>,
    #[prop(default = false)] submenu: Prop<bool>,
) -> NodeId {
    let theme = use_theme();
    let off = create_memo(move || disabled.get());
    let danger = create_memo(move || danger.get());
    let fill = create_memo(clone!(theme off -> move || {
        match (off.get(), active.get(), hovered.get()) {
            (true, _, _) => beui_core::color::Color32::TRANSPARENT,
            (false, true, _) => theme.pressed.get(),
            (false, false, true) => theme.hover.get(),
            (false, false, false) => beui_core::color::Color32::TRANSPARENT,
        }
    }));
    let text_color = create_memo(clone!(theme off danger -> move || {
        match (off.get(), danger.get()) {
            (true, _) => theme.text_muted.get(),
            (false, true) => theme.danger.get(),
            (false, false) => theme.text.get(),
        }
    }));
    let icon_color = create_memo(clone!(theme off -> move || {
        match (off.get(), danger.get()) {
            (false, true) => theme.danger.get(),
            _ => theme.text_muted.get(),
        }
    }));
    let glyph = create_memo(move || glyph.get());
    let has_glyph = create_memo(clone!(glyph -> move || !glyph.get().is_empty()));
    let detail = create_memo(move || detail.get());
    let chevron_color = theme.text_muted.clone();
    let has_detail = create_memo(clone!(detail -> move || !detail.get().is_empty()));
    view! {
        <Frame
            height=ROW_HEIGHT
            color={fill}
            radius=RADIUS
            outline={theme.accent.clone()}
            outline_width=FOCUS_RING_WIDTH
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
                <Show condition={submenu}>
                    <Icon glyph={ICON_CHEVRON_RIGHT.to_owned()} color={chevron_color.clone()} />
                </Show>
            </List>
        </Frame>
    }
}

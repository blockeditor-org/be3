use crate::frame::{Border, ColorRole, Frame, Radius, focus_border};
use beui_core::font::Overflow;
use beui_core::icons::Glyph;
use beui_macros::{component, view};

use crate::context_menu::{GlyphSlot, MenuTreeLead, SHEET_TREE_CHEVRON, SHEET_TREE_INDENT};
use crate::text::{Icon, Text, TextColor, TextSize};
use crate::theme::use_theme;
use beui_components_unstyled as unstyled;
use beui_core::font::TextAlign;
use beui_core::icons::{ICON_CHECK, ICON_CHEVRON_RIGHT, ICON_DOT};
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const ROW_HEIGHT: f32 = 52.0;
const PADDING_HORIZONTAL: f32 = 12.0;
const SPACING: f32 = 14.0;

#[component]
pub fn ActionRow(
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = None)] glyph: Prop<Option<Glyph>>,
    #[prop(default = None)] glyph_color: Prop<Option<TextColor>>,
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
            on_click={on_click}
        >|handle: unstyled::ButtonHandle| {
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
                <ActionRowFace
                    hovered
                    active
                    focused
                    label
                    glyph
                    glyph_color
                    detail
                    disabled
                    danger
                />
            }
        }</unstyled::Button>
    }
}

#[component]
pub(crate) fn ActionRowFace(
    hovered: ReadSignal<bool>,
    active: ReadSignal<bool>,
    focused: ReadSignal<bool>,
    label: Prop<String>,
    #[prop(default = String::new())] lead: Prop<String>,
    glyph: Prop<Option<Glyph>>,
    #[prop(default = None)] glyph_color: Prop<Option<TextColor>>,
    detail: Prop<String>,
    disabled: Prop<bool>,
    danger: Prop<bool>,
    #[prop(default = false)] submenu: Prop<bool>,
    #[prop(default = None)] checked: Prop<Option<bool>>,
    #[prop(default = false)] radio: bool,
    #[prop(default = false)] glyphs: Prop<bool>,
    #[prop(default = None)] depth: Prop<Option<usize>>,
    #[prop(default = None)] expanded: Prop<Option<bool>>,
    toggle: ClickCallback,
) -> NodeId {
    let theme = use_theme();
    let ticked = create_memo(move || checked.get() == Some(true));
    let tick = match radio {
        true => ICON_DOT,
        false => ICON_CHECK,
    };
    let off = create_memo(move || disabled.get());
    let danger = create_memo(move || danger.get());
    let fill = create_memo(move || match (off.get(), active.get(), hovered.get()) {
        (true, _, _) => beui_core::color::Color32::TRANSPARENT,
        (false, true, _) => theme.pressed.get(),
        (false, false, true) => theme.hover.get(),
        (false, false, false) => beui_core::color::Color32::TRANSPARENT,
    });
    let text_color = create_memo(move || match (off.get(), danger.get()) {
        (true, _) => TextColor::Muted,
        (false, true) => TextColor::Danger,
        (false, false) => TextColor::Default,
    });
    let icon_color = create_memo(move || match (off.get(), danger.get()) {
        (true, _) => TextColor::Muted,
        (false, true) => TextColor::Danger,
        _ => glyph_color.get().unwrap_or(TextColor::Muted),
    });
    let glyph = create_memo(move || glyph.get());
    let has_glyph = create_memo(move || glyph.get().is_some() || glyphs.get());
    let lead = create_memo(move || lead.get());
    let has_lead = create_memo(move || !lead.get().is_empty());
    let detail = create_memo(move || detail.get());
    let chevron_color = TextColor::Muted;
    let has_detail = create_memo(move || !detail.get().is_empty());
    view! {
        <Frame
            height=ROW_HEIGHT
            fill={ColorRole::custom(fill)}
            radius=Radius::Md
            border={focus_border(focused, Border::FOCUS)}
            padding={Sides::horizontal(PADDING_HORIZONTAL)}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <MenuTreeLead
                    depth
                    expanded
                    toggle
                    indent=SHEET_TREE_INDENT
                    chevron=SHEET_TREE_CHEVRON
                />
                <Show condition={has_lead}>
                    <Text size=TextSize::Sm color=TextColor::Muted>{lead}</Text>
                </Show>
                <Show condition={has_glyph}>
                    <GlyphSlot glyph color={icon_color} />
                </Show>
                <List @sizing=ItemSize::Percent(100.0) spacing=2.0>
                    <Text color={text_color} overflow=Overflow::Ellipsis>{label}</Text>
                    <Show condition={has_detail}>
                        <Text size=TextSize::Sm color=TextColor::Muted overflow=Overflow::Ellipsis>
                            {detail}
                        </Text>
                    </Show>
                </List>
                <Show condition={ticked}>
                    <Text color=TextColor::Accent align=TextAlign::Center>
                        <Icon glyph={tick.to_owned()} />
                    </Text>
                </Show>
                <Show condition={submenu}>
                    <Text color={chevron_color} align=TextAlign::Center>
                        <Icon glyph=ICON_CHEVRON_RIGHT />
                    </Text>
                </Show>
            </List>
        </Frame>
    }
}

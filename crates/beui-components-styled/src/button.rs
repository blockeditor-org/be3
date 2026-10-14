use beui_macros::{component, view};

use beui_core::color::Color32;

use crate::focus_ring::FocusRing;
use crate::text::{Icon, Text, TextColor};
use crate::theme::{BORDER_WIDTH, RADIUS, ThemeStore, use_theme};
use beui_components_unstyled as unstyled;
use beui_core::base::{ItemSize, TextAlign};
use beui_core::node::NodeId;
use beui_view::reactive::{
    Action, Align, ClickCallback, Direction, Frame, List, Prop, Show, create_memo,
};

const GLYPH_SPACING: f32 = 6.0;
const PADDING_HORIZONTAL: f32 = 16.0;
const PADDING_VERTICAL: f32 = 9.0;
const FOCUS_RING_OFFSET: f32 = 6.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    Primary,
    Secondary,
    Ghost,
}

impl ButtonVariant {
    pub fn fill(self, theme: &ThemeStore, disabled: bool, hovered: bool, active: bool) -> Color32 {
        if disabled {
            return match self {
                ButtonVariant::Primary => theme.accent_soft.get(),
                ButtonVariant::Secondary => theme.surface.get(),
                ButtonVariant::Ghost => Color32::TRANSPARENT,
            };
        }
        match (self, hovered, active) {
            (ButtonVariant::Primary, _, true) => theme.accent_active.get(),
            (ButtonVariant::Primary, true, false) => theme.accent_hover.get(),
            (ButtonVariant::Primary, false, false) => theme.accent.get(),
            (ButtonVariant::Secondary, _, true) => theme.pressed.get(),
            (ButtonVariant::Secondary, true, false) => theme.hover.get(),
            (ButtonVariant::Secondary, false, false) => theme.surface.get(),
            (ButtonVariant::Ghost, _, true) => theme.pressed.get(),
            (ButtonVariant::Ghost, true, false) => theme.hover.get(),
            (ButtonVariant::Ghost, false, false) => Color32::TRANSPARENT,
        }
    }

    pub fn label(self, disabled: bool) -> TextColor {
        if disabled {
            return TextColor::Muted;
        }
        match self {
            ButtonVariant::Primary => TextColor::OnAccent,
            ButtonVariant::Secondary | ButtonVariant::Ghost => TextColor::Default,
        }
    }
}

#[component]
pub fn Button(
    #[prop(default = String::new())] label: Prop<String>,
    variant: ButtonVariant,
    #[prop(default = String::new())] glyph: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] focused: Prop<bool>,
    action: Option<Action>,
    on_click: ClickCallback,
) -> NodeId {
    view! {
        <unstyled::Button
            label
            glyph
            action
            disabled
            focused
            on_click={on_click}
            content={move |handle| view! {
                <ButtonFace handle variant />
            }}
        />
    }
}

#[component]
pub fn ButtonFace(
    handle: unstyled::ButtonHandle,
    variant: ButtonVariant,
    #[prop(default = String::new())] trailing_glyph: Prop<String>,
    #[prop(default = false)] icon_only: Prop<bool>,
) -> NodeId {
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
    let fill_color =
        create_memo(move || variant.fill(&theme, disabled.get(), hovered.get(), active.get()));
    let label_color = create_memo(move || variant.label(disabled.get()));
    let icon_color = label_color;
    let has_glyph = create_memo(move || !glyph.get().is_empty());
    let shown_label = create_memo(move || match icon_only.get() {
        true => String::new(),
        false => label.get(),
    });
    let trailing = create_memo(move || trailing_glyph.get());
    let has_trailing = create_memo(move || !trailing.get().is_empty());
    let trailing_color = label_color;
    view! {
        <FocusRing focused radius={RADIUS + 4} offset=FOCUS_RING_OFFSET>
            <Frame
                color={fill_color}
                outline={theme.border}
                outline_width=BORDER_WIDTH
                radius=RADIUS
                outline_visible={variant == ButtonVariant::Secondary}
                padding_horizontal=PADDING_HORIZONTAL
                padding_vertical=PADDING_VERTICAL
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=GLYPH_SPACING>
                    <Show condition={has_glyph}>
                        <Text color={icon_color} align=TextAlign::Center>
                            <Icon glyph={glyph} />
                        </Text>
                    </Show>
                    <Text
                        @sizing={ItemSize::Intrinsic.shrink(1.0)}
                        string={shown_label}
                        color={label_color}
                        align=TextAlign::Center
                        ellipsis=true
                    />
                    <Show condition={has_trailing}>
                        <Text color={trailing_color} align=TextAlign::Center>
                            <Icon glyph={trailing} />
                        </Text>
                    </Show>
                </List>
            </Frame>
        </FocusRing>
    }
}

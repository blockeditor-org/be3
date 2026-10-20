use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_macros::{component, view};

use beui_core::color::Color32;

use crate::focus_ring::FocusRing;
use crate::text::{Icon, Text, TextColor};
use crate::theme::{ThemeStore, use_theme};
use beui_components_unstyled as unstyled;
use beui_core::base::{ItemSize, TextAlign};
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const GLYPH_SPACING: f32 = 6.0;
const PADDING_HORIZONTAL: f32 = 16.0;
const PADDING_VERTICAL: f32 = 9.0;
const FOCUS_RING_OFFSET: f32 = 6.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    Primary,
    Secondary,
    Danger,
    Ghost,
}

impl ButtonVariant {
    pub fn fill(self, theme: &ThemeStore, disabled: bool, hovered: bool, active: bool) -> Color32 {
        if disabled {
            return match self {
                ButtonVariant::Primary => theme.accent_soft.get(),
                ButtonVariant::Danger => theme.danger_soft.get(),
                ButtonVariant::Secondary => theme.surface.get(),
                ButtonVariant::Ghost => Color32::TRANSPARENT,
            };
        }
        match (self, hovered, active) {
            (ButtonVariant::Primary, _, true) => theme.accent_active.get(),
            (ButtonVariant::Primary, true, false) => theme.accent_hover.get(),
            (ButtonVariant::Primary, false, false) => theme.accent.get(),
            (ButtonVariant::Danger, _, true) => theme.danger_fill_active.get(),
            (ButtonVariant::Danger, true, false) => theme.danger_fill_hover.get(),
            (ButtonVariant::Danger, false, false) => theme.danger_fill.get(),
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
            ButtonVariant::Danger => TextColor::OnDanger,
            ButtonVariant::Secondary | ButtonVariant::Ghost => TextColor::Default,
        }
    }
}

#[component]
pub fn Button(
    #[prop(default = String::new())] label: Prop<String>,
    variant: Prop<ButtonVariant>,
    #[prop(default = String::new())] glyph: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] focused: Prop<bool>,
    action: Option<Action>,
    on_click: ClickCallback,
) -> NodeId {
    view! {
        <unstyled::Button label glyph action disabled focused on_click={on_click}>|handle|
            <ButtonFace handle variant />
        </unstyled::Button>
    }
}

#[component]
pub fn ButtonFace(
    handle: unstyled::ButtonHandle,
    variant: Prop<ButtonVariant>,
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
    let fill_color = create_memo(move || {
        variant
            .get()
            .fill(&theme, disabled.get(), hovered.get(), active.get())
    });
    let label_color = create_memo(move || variant.get().label(disabled.get()));
    let border =
        create_memo(move || (variant.get() == ButtonVariant::Secondary).then_some(Border::DEFAULT));
    let has_glyph = create_memo(move || !glyph.get().is_empty());
    let trailing = create_memo(move || trailing_glyph.get());
    let has_trailing = create_memo(move || !trailing.get().is_empty());
    let labelled = create_memo(move || !icon_only.get());
    let padding = create_memo(move || match icon_only.get() {
        true => Sides::symmetric(PADDING_VERTICAL, PADDING_VERTICAL),
        false => Sides::symmetric(PADDING_HORIZONTAL, PADDING_VERTICAL),
    });
    view! {
        <FocusRing focused radius={Radius::custom(Radius::Md.px() + 4.0)} offset=FOCUS_RING_OFFSET>
            <Frame
                fill={ColorRole::custom(fill_color)}
                border={border}
                radius=Radius::Md
                padding={padding}
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=GLYPH_SPACING>
                    <Show condition={has_glyph}>
                        <Text color={label_color} align=TextAlign::Center>
                            <Icon glyph={glyph} />
                        </Text>
                    </Show>
                    <Show condition={labelled}>
                        <Text
                            @sizing={ItemSize::Intrinsic.shrink(1.0)}
                            string={label}
                            color={label_color}
                            align=TextAlign::Center
                            ellipsis=true
                        />
                    </Show>
                    <Show condition={has_trailing}>
                        <Text color={label_color} align=TextAlign::Center>
                            <Icon glyph={trailing} />
                        </Text>
                    </Show>
                </List>
            </Frame>
        </FocusRing>
    }
}

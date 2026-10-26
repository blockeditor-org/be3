use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_core::font::Overflow;
use beui_core::icons::Glyph;
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
const COMPACT_PADDING_HORIZONTAL: f32 = 11.0;
const COMPACT_PADDING_VERTICAL: f32 = 7.0;
const COMPACT_RADIUS: f32 = 8.0;
const FOCUS_RING_OFFSET: f32 = 6.0;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum ButtonSize {
    #[default]
    Regular,
    Compact,
}

impl ButtonSize {
    fn padding(self) -> (f32, f32) {
        match self {
            ButtonSize::Regular => (PADDING_HORIZONTAL, PADDING_VERTICAL),
            ButtonSize::Compact => (COMPACT_PADDING_HORIZONTAL, COMPACT_PADDING_VERTICAL),
        }
    }

    fn radius(self) -> f32 {
        match self {
            ButtonSize::Regular => Radius::Md.px(),
            ButtonSize::Compact => COMPACT_RADIUS,
        }
    }
}

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
    #[prop(default = ButtonSize::Regular)] size: ButtonSize,
    #[prop(default = None)] glyph: Prop<Option<Glyph>>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] focused: Prop<bool>,
    action: Option<Action>,
    on_click: ClickCallback,
) -> NodeId {
    view! {
        <unstyled::Button label glyph action disabled focused on_click={on_click}>|handle|
            <ButtonFace handle variant size />
        </unstyled::Button>
    }
}

#[component]
pub fn ButtonFace(
    handle: unstyled::ButtonHandle,
    variant: Prop<ButtonVariant>,
    #[prop(default = ButtonSize::Regular)] size: ButtonSize,
    #[prop(default = None)] trailing_glyph: Prop<Option<Glyph>>,
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
    let has_glyph = create_memo(move || glyph.get().is_some());
    let trailing = create_memo(move || trailing_glyph.get());
    let has_trailing = create_memo(move || trailing.get().is_some());
    let labelled = create_memo(move || !icon_only.get());
    let (horizontal, vertical) = size.padding();
    let padding = create_memo(move || match icon_only.get() {
        true => Sides::symmetric(vertical, vertical),
        false => Sides::symmetric(horizontal, vertical),
    });
    view! {
        <FocusRing focused radius={Radius::custom(size.radius() + 4.0)} offset=FOCUS_RING_OFFSET>
            <Frame
                fill={ColorRole::custom(fill_color)}
                border={border}
                radius={Radius::custom(size.radius())}
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
                            color={label_color}
                            align=TextAlign::Center
                            overflow=Overflow::Ellipsis
                        >
                            {label}
                        </Text>
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

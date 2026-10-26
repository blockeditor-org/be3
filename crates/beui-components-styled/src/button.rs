use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_core::font::Overflow;
use beui_core::icons::Glyph;
use beui_macros::{component, view};

use beui_core::color::Color32;

use crate::focus_ring::FocusRing;
use crate::text::{Icon, Text, TextColor, TextSize};
use crate::theme::{ThemeStore, use_theme};
use crate::tooltip::Tooltip;
use beui_components_unstyled::{self as unstyled, Picture};
use beui_core::base::frame::{Align2, Extent};
use beui_core::base::{ItemSize, TextAlign};
use beui_core::image::Image;
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const GLYPH_SPACING: f32 = 6.0;
const TOOLBAR_SIDE: f32 = 30.0;
const TOOLBAR_RADIUS: f32 = 7.0;
const COMPACT_RADIUS: f32 = 8.0;
const FOCUS_RING_GROWTH: f32 = 4.0;

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum ButtonSize {
    Small,
    Compact,
    #[default]
    Regular,
    Toolbar,
}

impl ButtonSize {
    fn padding(self, icon_only: bool) -> Sides {
        let (horizontal, vertical) = match (self, icon_only) {
            (ButtonSize::Small, true) => (2.0, 2.0),
            (ButtonSize::Small, false) => (6.0, 2.0),
            (ButtonSize::Compact, true) => (6.0, 6.0),
            (ButtonSize::Compact, false) => (11.0, 7.0),
            (ButtonSize::Regular, true) => (8.0, 8.0),
            (ButtonSize::Regular, false) => (16.0, 9.0),
            (ButtonSize::Toolbar, true) => (0.0, 0.0),
            (ButtonSize::Toolbar, false) => (10.0, 0.0),
        };
        Sides::symmetric(horizontal, vertical)
    }

    fn width(self, icon_only: bool) -> Extent {
        match (self, icon_only) {
            (ButtonSize::Toolbar, true) => Extent::px(TOOLBAR_SIDE),
            _ => Extent::INTRINSIC,
        }
    }

    fn height(self) -> Extent {
        match self {
            ButtonSize::Toolbar => Extent::px(TOOLBAR_SIDE),
            _ => Extent::INTRINSIC,
        }
    }

    fn align(self) -> Align2 {
        match self {
            ButtonSize::Toolbar => Align2::CENTER,
            _ => Align2::STRETCH,
        }
    }

    fn radius(self) -> f32 {
        match self {
            ButtonSize::Small | ButtonSize::Regular => Radius::Md.px(),
            ButtonSize::Compact => COMPACT_RADIUS,
            ButtonSize::Toolbar => TOOLBAR_RADIUS,
        }
    }

    fn text(self) -> TextSize {
        match self {
            ButtonSize::Small => TextSize::Xs,
            _ => TextSize::Base,
        }
    }

    fn focus_ring_offset(self, icon_only: bool) -> f32 {
        match (self, icon_only) {
            (ButtonSize::Small, _) => 1.0,
            (ButtonSize::Compact, true) => 2.0,
            (_, true) => 4.0,
            (_, false) => 6.0,
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
    #[prop(default = ButtonVariant::Secondary)] variant: Prop<ButtonVariant>,
    #[prop(default = ButtonSize::Regular)] size: ButtonSize,
    #[prop(default = None)] glyph: Prop<Option<Glyph>>,
    #[prop(default = false)] icon_only: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = false)] capture_presses: Prop<bool>,
    #[prop(default = true)] press_focus: Prop<bool>,
    action: Option<Action>,
    on_click: ClickCallback,
) -> NodeId {
    view! {
        <unstyled::Button
            label
            glyph
            icon_only
            action
            disabled
            focused
            capture_presses
            press_focus
            on_click={on_click}
        >|handle|
            <ButtonFace handle variant size />
        </unstyled::Button>
    }
}

#[component]
pub fn ButtonFace(
    handle: unstyled::ButtonHandle,
    #[prop(default = ButtonVariant::Secondary)] variant: Prop<ButtonVariant>,
    #[prop(default = ButtonSize::Regular)] size: ButtonSize,
    #[prop(default = None)] trailing_glyph: Prop<Option<Glyph>>,
    #[prop(default = None)] image: Prop<Option<Image>>,
    #[prop(default = false)] pressed: Prop<bool>,
) -> NodeId {
    let unstyled::ButtonHandle {
        hovered,
        active,
        focused,
        disabled,
        label,
        glyph,
        icon_only,
        tooltip,
    } = handle;
    let theme = use_theme();
    let fill_color = create_memo(move || {
        let held = active.get() || pressed.get();
        variant
            .get()
            .fill(&theme, disabled.get(), hovered.get(), held)
    });
    let label_color = create_memo(move || variant.get().label(disabled.get()));
    let border =
        create_memo(move || (variant.get() == ButtonVariant::Secondary).then_some(Border::DEFAULT));
    let image = create_memo(move || image.get());
    let pictured = create_memo(move || image.with(Option::is_some));
    let has_glyph = create_memo(move || !pictured.get() && glyph.get().is_some());
    let trailing = create_memo(move || trailing_glyph.get());
    let has_trailing = create_memo(move || trailing.get().is_some());
    let labelled = create_memo(move || !icon_only.get());
    let padding = create_memo(move || size.padding(icon_only.get()));
    let width = create_memo(move || size.width(icon_only.get()));
    let offset = create_memo(move || size.focus_ring_offset(icon_only.get()));
    let text = size.text();
    let picture = text.icon_px();
    view! {
        <Tooltip label={tooltip}>
            <FocusRing
                focused
                radius={Radius::custom(size.radius() + FOCUS_RING_GROWTH)}
                offset={offset}
            >
                <Frame
                    fill={ColorRole::custom(fill_color)}
                    border={border}
                    radius={Radius::custom(size.radius())}
                    width={width}
                    height={size.height()}
                    align={size.align()}
                    padding={padding}
                >
                    <List direction=Direction::Horizontal align=Align::Center spacing=GLYPH_SPACING>
                        <Show condition={pictured}>
                            <Frame width=picture height=picture>
                                <Picture image />
                            </Frame>
                        </Show>
                        <Show condition={has_glyph}>
                            <Text size=text color={label_color} align=TextAlign::Center>
                                <Icon glyph={glyph} />
                            </Text>
                        </Show>
                        <Show condition={labelled}>
                            <Text
                                @sizing={ItemSize::Intrinsic.shrink(1.0)}
                                size=text
                                color={label_color}
                                align=TextAlign::Center
                                overflow=Overflow::Ellipsis
                            >
                                {label}
                            </Text>
                        </Show>
                        <Show condition={has_trailing}>
                            <Text size=text color={label_color} align=TextAlign::Center>
                                <Icon glyph={trailing} />
                            </Text>
                        </Show>
                    </List>
                </Frame>
            </FocusRing>
        </Tooltip>
    }
}

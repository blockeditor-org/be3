use beui_core::icons::Glyph;
use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_macros::{component, view};

use crate::button::ButtonVariant;
use crate::focus_ring::FocusRing;
use crate::text::{Icon, Text, TextSize};
use crate::theme::use_theme;
use crate::tooltip::Tooltip;
use beui_components_unstyled as unstyled;
use beui_core::base::frame::{Align2, Extent};
use beui_core::font::TextAlign;
use beui_core::node::NodeId;
use beui_view::reactive::*;

const PADDING: f32 = 8.0;
const COMPACT_PADDING: f32 = 2.0;
const TOOLBAR_SIDE: f32 = 30.0;
const TOOLBAR_RADIUS: f32 = 7.0;
const FOCUS_RING_OFFSET: f32 = 4.0;
const COMPACT_FOCUS_RING_OFFSET: f32 = 1.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IconButtonSize {
    Compact,
    Regular,
    Toolbar,
}

impl IconButtonSize {
    fn padding(self) -> f32 {
        match self {
            IconButtonSize::Compact => COMPACT_PADDING,
            IconButtonSize::Regular => PADDING,
            IconButtonSize::Toolbar => 0.0,
        }
    }

    fn side(self) -> Extent {
        match self {
            IconButtonSize::Compact | IconButtonSize::Regular => Extent::INTRINSIC,
            IconButtonSize::Toolbar => Extent::px(TOOLBAR_SIDE),
        }
    }

    fn radius(self) -> f32 {
        match self {
            IconButtonSize::Compact | IconButtonSize::Regular => Radius::Md.px(),
            IconButtonSize::Toolbar => TOOLBAR_RADIUS,
        }
    }

    fn glyph(self) -> TextSize {
        match self {
            IconButtonSize::Compact => TextSize::Xs,
            IconButtonSize::Regular | IconButtonSize::Toolbar => TextSize::Base,
        }
    }

    fn focus_ring_offset(self) -> f32 {
        match self {
            IconButtonSize::Compact => COMPACT_FOCUS_RING_OFFSET,
            IconButtonSize::Regular | IconButtonSize::Toolbar => FOCUS_RING_OFFSET,
        }
    }
}

#[component]
pub fn IconButton(
    #[prop(default = None)] glyph: Prop<Option<Glyph>>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = ButtonVariant::Secondary)] variant: ButtonVariant,
    #[prop(default = IconButtonSize::Regular)] size: IconButtonSize,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] capture_presses: Prop<bool>,
    #[prop(default = true)] press_focus: Prop<bool>,
    action: Option<Action>,
    on_click: ClickCallback,
) -> NodeId {
    view! {
        <unstyled::Button
            label
            glyph
            action
            disabled
            capture_presses
            press_focus
            on_click={on_click}
        >|handle: unstyled::ButtonHandle|
            <Tooltip label={handle.tooltip}>
                <IconButtonFace handle variant size />
            </Tooltip>
        </unstyled::Button>
    }
}

#[component]
pub(crate) fn IconButtonFace(
    handle: unstyled::ButtonHandle,
    variant: ButtonVariant,
    size: IconButtonSize,
) -> NodeId {
    let unstyled::ButtonHandle {
        hovered,
        active,
        focused,
        disabled,
        glyph,
        ..
    } = handle;
    let theme = use_theme();
    let fill_color =
        create_memo(move || variant.fill(&theme, disabled.get(), hovered.get(), active.get()));
    let icon_color = create_memo(move || variant.label(disabled.get()));
    view! {
        <FocusRing
            focused
            radius={Radius::custom(size.radius() + 4.0)}
            offset={size.focus_ring_offset()}
        >
            <Frame
                fill={ColorRole::custom(fill_color)}
                border={(variant == ButtonVariant::Secondary).then_some(Border::DEFAULT)}
                radius={Radius::custom(size.radius())}
                width={size.side()}
                height={size.side()}
                align=Align2::CENTER
                padding={size.padding()}
            >
                <Text size={size.glyph()} color={icon_color} align=TextAlign::Center>
                    <Icon glyph={glyph} />
                </Text>
            </Frame>
        </FocusRing>
    }
}

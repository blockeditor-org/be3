use beui_macros::{component, view};

use crate::button::ButtonVariant;
use crate::focus_ring::FocusRing;
use crate::text::IconSized;
use crate::theme::{BORDER_WIDTH, FONT_BODY, ICON_SIZE, RADIUS, use_theme};
use crate::tooltip::Tooltip;
use beui_components_unstyled as unstyled;
use beui_core::node::NodeId;
use beui_view::reactive::{Action, ClickCallback, Frame, Prop, clone, create_memo};

const PADDING: f32 = 8.0;
const COMPACT_PADDING: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 4.0;
const COMPACT_FOCUS_RING_OFFSET: f32 = 1.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IconButtonSize {
    Compact,
    Regular,
}

impl IconButtonSize {
    fn padding(self) -> f32 {
        match self {
            IconButtonSize::Compact => COMPACT_PADDING,
            IconButtonSize::Regular => PADDING,
        }
    }

    fn glyph(self) -> f32 {
        match self {
            IconButtonSize::Compact => FONT_BODY,
            IconButtonSize::Regular => ICON_SIZE,
        }
    }

    fn focus_ring_offset(self) -> f32 {
        match self {
            IconButtonSize::Compact => COMPACT_FOCUS_RING_OFFSET,
            IconButtonSize::Regular => FOCUS_RING_OFFSET,
        }
    }
}

#[component]
pub fn IconButton(
    #[prop(default = String::new())] glyph: Prop<String>,
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
            on_click={move || on_click.call()}
            content={move |handle: unstyled::ButtonHandle| view! {
                <Tooltip label={handle.tooltip.clone()}>
                    <IconButtonFace handle variant size />
                </Tooltip>
            }}
        />
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
    let fill_color = create_memo(clone!(theme disabled -> move || {
        variant.fill(&theme, disabled.get(), hovered.get(), active.get())
    }));
    let icon_color = create_memo(clone!(theme -> move || variant.label(&theme, disabled.get())));
    view! {
        <FocusRing focused radius={RADIUS + 4} offset={size.focus_ring_offset()}>
            <Frame
                color={fill_color}
                outline={theme.border.clone()}
                outline_width=BORDER_WIDTH
                radius=RADIUS
                outline_visible={variant == ButtonVariant::Secondary}
                padding_horizontal={size.padding()}
                padding_vertical={size.padding()}
            >
                <IconSized glyph={glyph} font_size={size.glyph()} color={icon_color} />
            </Frame>
        </FocusRing>
    }
}

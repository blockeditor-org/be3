use beui_macros::{component, view};

use crate::color::Color32;

use crate::base::TextAlign;
use crate::node::NodeId;
use crate::reactive::{create_memo, ClickCallback, Frame, Prop, Text};
use crate::styled::theme::{
    ACCENT, ACCENT_ACTIVE, ACCENT_HOVER, BORDER, BORDER_WIDTH, FONT_BODY, ON_ACCENT, RADIUS,
    SURFACE, SURFACE_RAISED, TEXT,
};
use crate::unstyled;

const PADDING_HORIZONTAL: f32 = 16.0;
const PADDING_VERTICAL: f32 = 9.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 6.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    Primary,
    Secondary,
}

impl ButtonVariant {
    pub(crate) fn fill(self, hovered: bool, active: bool) -> Color32 {
        match (self, hovered, active) {
            (ButtonVariant::Primary, _, true) => ACCENT_ACTIVE,
            (ButtonVariant::Primary, true, false) => ACCENT_HOVER,
            (ButtonVariant::Primary, false, false) => ACCENT,
            (ButtonVariant::Secondary, _, true) => BORDER,
            (ButtonVariant::Secondary, true, false) => SURFACE_RAISED,
            (ButtonVariant::Secondary, false, false) => SURFACE,
        }
    }

    pub(crate) fn label(self) -> Color32 {
        match self {
            ButtonVariant::Primary => ON_ACCENT,
            ButtonVariant::Secondary => TEXT,
        }
    }
}

#[component]
pub fn Button(
    label: Prop<String>,
    variant: ButtonVariant,
    disabled: Prop<bool>,
    on_click: ClickCallback,
) -> NodeId {
    view! {
        <unstyled::Button
            disabled
            on_click={move || on_click.call()}
            content={move |handle| view! {
                <ButtonFace handle variant label />
            }}
        />
    }
}

#[component]
fn ButtonFace(
    handle: unstyled::ButtonHandle,
    variant: ButtonVariant,
    label: Prop<String>,
) -> NodeId {
    let unstyled::ButtonHandle {
        hovered,
        active,
        focused,
    } = handle;
    let fill_color = create_memo(move || variant.fill(hovered.get(), active.get()));
    view! {
        <Frame outline=ACCENT outline_width=FOCUS_RING_WIDTH radius={RADIUS + 4} outline_offset=FOCUS_RING_OFFSET outline_visible={focused}>
            <Frame
                color={fill_color}
                outline=BORDER
                outline_width=BORDER_WIDTH
                radius=RADIUS
                outline_visible={variant == ButtonVariant::Secondary}
                padding_horizontal=PADDING_HORIZONTAL
                padding_vertical=PADDING_VERTICAL
            >
                <Text string={label} font_size=FONT_BODY color={variant.label()} align=TextAlign::Center />
            </Frame>
        </Frame>
    }
}

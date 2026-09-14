use beui_macros::{component, view};

use crate::color::Color32;

use crate::base::TextAlign;
use crate::node::NodeId;
use crate::reactive::{ClickCallback, Frame, Prop, Text, clone, create_memo};
use crate::styled::theme::{BORDER_WIDTH, FONT_BODY, RADIUS, Theme, use_theme};
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
    pub(crate) fn fill(self, theme: &Theme, hovered: bool, active: bool) -> Color32 {
        match (self, hovered, active) {
            (ButtonVariant::Primary, _, true) => theme.accent_active,
            (ButtonVariant::Primary, true, false) => theme.accent_hover,
            (ButtonVariant::Primary, false, false) => theme.accent,
            (ButtonVariant::Secondary, _, true) => theme.pressed,
            (ButtonVariant::Secondary, true, false) => theme.hover,
            (ButtonVariant::Secondary, false, false) => theme.surface,
        }
    }

    pub(crate) fn label(self, theme: &Theme) -> Color32 {
        match self {
            ButtonVariant::Primary => theme.on_accent,
            ButtonVariant::Secondary => theme.text,
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
    let theme = use_theme();
    let fill_color = create_memo(
        clone!(theme -> move || variant.fill(&theme.get(), hovered.get(), active.get())),
    );
    view! {
        <Frame
            outline={theme.pick(|theme| theme.accent)}
            outline_width=FOCUS_RING_WIDTH
            radius={RADIUS + 4}
            outline_offset=FOCUS_RING_OFFSET
            outline_visible={focused}
        >
            <Frame
                color={fill_color}
                outline={theme.pick(|theme| theme.border)}
                outline_width=BORDER_WIDTH
                radius=RADIUS
                outline_visible={variant == ButtonVariant::Secondary}
                padding_horizontal=PADDING_HORIZONTAL
                padding_vertical=PADDING_VERTICAL
            >
                <Text
                    string={label}
                    font_size=FONT_BODY
                    color={theme.pick(move |theme| variant.label(theme))}
                    align=TextAlign::Center
                />
            </Frame>
        </Frame>
    }
}

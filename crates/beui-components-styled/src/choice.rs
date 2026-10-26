use crate::frame::{Border, ColorRole, Frame, Radius, focus_border};
use beui_core::base::TextAlign;
use beui_core::color::Color32;
use beui_core::font::Overflow;
use beui_core::node::NodeId;
use beui_macros::{component, view};

use crate::theme::{ThemeStore, use_theme};
use beui_components_unstyled::ChoiceOptionHandle;
use beui_view::primitives::*;
use beui_view::reactive::*;

use crate::text::{Text, TextColor};
pub(super) use beui_components_unstyled::ChoiceKind as Kind;

const MARK_SPACING: f32 = 10.0;
const MARK_BOX: f32 = 18.0;
const MARK_DOT: f32 = 8.0;
const MARK_RADIUS: f32 = 9.0;

#[component]
pub(super) fn OptionFace(kind: Kind, handle: ChoiceOptionHandle) -> NodeId {
    let ChoiceOptionHandle {
        label,
        selected,
        hovered,
        focused,
        ..
    } = handle;
    let theme = use_theme();
    let label_color = create_memo(move || match selected.get() {
        true => TextColor::Default,
        false => TextColor::Muted,
    });
    let fill_color = create_memo(move || background(&theme, selected.get(), hovered.get()));
    view! {
        <Frame
            fill={ColorRole::custom(fill_color)}
            border={focus_border(focused, Border::FOCUS.offset(1.0))}
            radius=Radius::Md
            padding={Sides::symmetric(14.0, 6.0)}
        >
            <ChoiceLabel kind label color={label_color} checked={selected} />
        </Frame>
    }
}

#[component]
fn ChoiceLabel(
    kind: Kind,
    label: Prop<String>,
    color: Prop<TextColor>,
    checked: Memo<bool>,
) -> NodeId {
    let align = if kind == Kind::Tabs {
        TextAlign::Center
    } else {
        TextAlign::Start
    };
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=MARK_SPACING>
            <Show condition={kind == Kind::Radio}>
                <RadioMark checked={checked} />
            </Show>
            <Text overflow=Overflow::Ellipsis @sizing=ItemSize::Percent(100.0) color align>
                {label}
            </Text>
        </List>
    }
}

#[component]
fn RadioMark(checked: Memo<bool>) -> NodeId {
    let theme = use_theme();
    let ring_color = create_memo(move || ring(&theme, checked.get()));
    view! {
        <Frame
            width=MARK_BOX
            height=MARK_BOX
            border={create_memo(move || Some(Border::new(ColorRole::Custom(ring_color.get())).width(2.0)))}
            radius={Radius::custom(MARK_RADIUS)}
            align=Align::Center
        >
            <Layers>
                <Show condition={checked}>
                    <Frame
                        width=MARK_DOT
                        height=MARK_DOT
                        fill=ColorRole::Accent
                        radius={Radius::custom(MARK_RADIUS)}
                    />
                </Show>
            </Layers>
        </Frame>
    }
}

fn ring(theme: &ThemeStore, checked: bool) -> Color32 {
    if checked {
        theme.accent.get()
    } else {
        theme.border.get()
    }
}

fn background(theme: &ThemeStore, active: bool, hovered: bool) -> Color32 {
    match (active, hovered) {
        (true, _) => theme.accent_soft.get(),
        (false, true) => theme.hover.get(),
        _ => Color32::TRANSPARENT,
    }
}

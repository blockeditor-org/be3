use crate::frame::{Border, ColorRole, Frame, Radius, focus_border};
use beui_core::base::TextAlign;
use beui_core::color::Color32;
use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_macros::{component, view};

use crate::theme::{ThemeStore, use_theme};
use beui_components_unstyled as unstyled;
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
        state,
        ..
    } = handle;
    let theme = use_theme();
    let selected = create_memo(move || state.get().selected);
    let label_color = create_memo(move || match selected.get() {
        true => TextColor::Default,
        false => TextColor::Muted,
    });
    let fill_color = create_memo(move || {
        let state = state.get();
        background(&theme, state.selected, state.hovered)
    });
    view! {
        <Frame
            fill={ColorRole::custom(fill_color)}
            border={focus_border(Prop::dynamic(move || state.get().focused), Border::FOCUS.offset(1.0))}
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
            <Text @sizing=ItemSize::Percent(100.0) string={label} color align />
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
            <Frame
                visible={checked}
                width=MARK_DOT
                height=MARK_DOT
                fill=ColorRole::Accent
                radius={Radius::custom(MARK_RADIUS)}
            />
        </Frame>
    }
}

pub(super) fn selected_index(document: &Document, choice: NodeId) -> Option<usize> {
    unstyled::choice_selected(document, choice)
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

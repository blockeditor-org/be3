use crate::base::TextAlign;
use crate::color::Color32;
use crate::document::Document;
use crate::node::NodeId;
use beui_macros::{component, view};

use crate::reactive::Memo;
use crate::reactive::{CenteredRow, Frame, ItemSize, Prop, Spacer, Text, clone, create_memo};
use crate::styled::theme::{FONT_BODY, RADIUS, ThemeStore, use_theme};
use crate::unstyled;
use crate::unstyled::ChoiceOptionHandle;

pub(super) use crate::unstyled::ChoiceKind as Kind;

const MARK_SPACING: f32 = 10.0;
const MARK_BOX: f32 = 18.0;
const MARK_DOT: f32 = 8.0;
const MARK_RADIUS: u8 = 9;

#[component]
pub(super) fn ChoiceOption(kind: Kind, handle: ChoiceOptionHandle) -> NodeId {
    let ChoiceOptionHandle {
        label,
        selected,
        hovered,
        focused,
        ..
    } = handle;
    let theme = use_theme();
    let label_color = create_memo(clone!(selected theme -> move || {
        let theme = theme.get();
        if selected.get() {
            theme.text
        } else {
            theme.text_muted
        }
    }));
    let checked = selected.clone();
    let fill_color =
        create_memo(clone!(theme -> move || background(&theme, selected.get(), hovered.get())));
    view! {
        <Frame
            color=fill_color
            outline={theme.accent.clone()}
            outline_width=2.0
            radius=RADIUS
            outline_offset=1.0
            outline_visible=focused
            padding_horizontal=14.0
            padding_vertical=6.0
        >
            <ChoiceLabel kind label color=label_color checked />
        </Frame>
    }
}

#[component]
fn ChoiceLabel(kind: Kind, label: String, color: Prop<Color32>, checked: Memo<bool>) -> NodeId {
    let align = if kind == Kind::Tabs {
        TextAlign::Center
    } else {
        TextAlign::Start
    };
    if kind != Kind::Radio {
        return view! {
            <Text string=label font_size=FONT_BODY color align />
        };
    }
    view! {
        <CenteredRow spacing=MARK_SPACING>
            <RadioMark checked />
            <Text @sizing=ItemSize::Percent(100.0) string=label font_size=FONT_BODY color align />
        </CenteredRow>
    }
}

#[component]
fn RadioMark(checked: Memo<bool>) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame
            width=MARK_BOX
            height=MARK_BOX
            outline={theme.border.clone()}
            outline_width=2.0
            radius=MARK_RADIUS
            outline_visible=true
        >
            <CenteredRow spacing=0.0>
                <Spacer @sizing=ItemSize::Percent(100.0) />
                <Frame
                    visible=checked
                    width=MARK_DOT
                    height=MARK_DOT
                    color={theme.accent.clone()}
                    radius=MARK_RADIUS
                />
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </CenteredRow>
        </Frame>
    }
}

pub(super) fn selected_index(document: &Document, choice: NodeId) -> Option<usize> {
    unstyled::choice_selected(document, choice)
}

fn background(theme: &ThemeStore, active: bool, hovered: bool) -> Color32 {
    match (active, hovered) {
        (true, _) => theme.accent_soft.get(),
        (false, true) => theme.hover.get(),
        _ => Color32::TRANSPARENT,
    }
}

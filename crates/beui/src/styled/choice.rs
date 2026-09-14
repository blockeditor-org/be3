use crate::base::TextAlign;
use crate::color::Color32;
use crate::document::Document;
use crate::node::NodeId;
use beui_macros::{component, view};

use crate::reactive::Memo;
use crate::reactive::{clone, create_memo, CenteredRow, Frame, ItemSize, Prop, Spacer, Text};
use crate::styled::theme::{
    ACCENT, ACCENT_SOFT, BORDER, FONT_BODY, RADIUS, SURFACE_RAISED, TEXT, TEXT_MUTED,
};
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
    let label_color =
        create_memo(clone!(selected -> move || if selected.get() { TEXT } else { TEXT_MUTED }));
    let checked = selected.clone();
    let fill_color = create_memo(move || background(selected.get(), hovered.get()));
    view! {
        <Frame color={fill_color} outline=ACCENT outline_width=2.0 radius=RADIUS outline_offset=1.0 outline_visible={focused} padding_horizontal=14.0 padding_vertical=6.0>
            <ChoiceLabel kind label color={label_color} checked />
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
        return view! { <Text string={label} font_size=FONT_BODY color align /> };
    }
    view! {
        <CenteredRow spacing=MARK_SPACING>
            <RadioMark checked />
            <Text @sizing=ItemSize::Percent(100.0) string={label} font_size=FONT_BODY color align />
        </CenteredRow>
    }
}

#[component]
fn RadioMark(checked: Memo<bool>) -> NodeId {
    view! {
        <Frame width=MARK_BOX height=MARK_BOX outline=BORDER outline_width=2.0 radius=MARK_RADIUS outline_visible=true>
            <CenteredRow spacing=0.0>
                <Spacer @sizing=ItemSize::Percent(100.0) />
                <Frame visible={checked} width=MARK_DOT height=MARK_DOT color=ACCENT radius=MARK_RADIUS />
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </CenteredRow>
        </Frame>
    }
}

pub(super) fn selected_index(document: &Document, choice: NodeId) -> Option<usize> {
    unstyled::choice_selected(document, choice)
}

fn background(active: bool, hovered: bool) -> Color32 {
    match (active, hovered) {
        (true, _) => ACCENT_SOFT,
        (false, true) => SURFACE_RAISED,
        _ => Color32::TRANSPARENT,
    }
}

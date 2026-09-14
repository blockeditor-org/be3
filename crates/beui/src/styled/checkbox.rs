use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::color::Color32;

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    Callback, CenteredRow, Frame, ItemSize, Prop, Spacer, Text, clone, create_memo,
};
use crate::styled::theme::{BORDER_WIDTH, CHIP_RADIUS, FONT_BODY, RADIUS, Theme, use_theme};
use crate::unstyled;
use crate::unstyled::{Toggle, ToggleHandle};

const BOX_SIZE: f32 = 18.0;
const MARK_SIZE: f32 = 10.0;
const MARK_RADIUS: u8 = 2;
const SPACING: f32 = 10.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 4.0;

#[component]
pub fn Checkbox(label: Prop<String>, checked: Prop<bool>, on_change: Callback<bool>) -> NodeId {
    view! {
        <Toggle checked on_change={move |checked| on_change.call(checked)}>
            {move |handle: ToggleHandle| {
                view! { <CheckboxFace handle label /> }
            }}
        </Toggle>
    }
}

#[component]
fn CheckboxFace(handle: ToggleHandle, label: Prop<String>) -> NodeId {
    let ToggleHandle {
        checked,
        hovered,
        focused,
        ..
    } = handle;
    let theme = use_theme();
    let fill_color = create_memo(
        clone!(checked theme -> move || box_fill(&theme.get(), checked.get(), hovered.get())),
    );
    let border_visible = create_memo(clone!(checked -> move || !checked.get()));
    let box_border = theme.pick(|theme| theme.border);
    let mark_color = theme.pick(|theme| theme.on_accent);
    let label_color = theme.pick(|theme| theme.text);

    view! {
        <Frame
            outline={theme.pick(|theme| theme.accent)}
            outline_width=FOCUS_RING_WIDTH
            radius=RADIUS
            outline_offset=FOCUS_RING_OFFSET
            outline_visible={focused}
        >
            <CenteredRow spacing=SPACING>
                <Frame
                    width=BOX_SIZE
                    height=BOX_SIZE
                    color={fill_color}
                    outline={box_border}
                    outline_width=BORDER_WIDTH
                    radius=CHIP_RADIUS
                    outline_visible={border_visible}
                >
                    <CenteredRow spacing=0.0>
                        <Spacer @sizing=ItemSize::Percent(100.0) />
                        <Frame
                            visible={checked}
                            width=MARK_SIZE
                            height=MARK_SIZE
                            color={mark_color}
                            radius=MARK_RADIUS
                        />
                        <Spacer @sizing=ItemSize::Percent(100.0) />
                    </CenteredRow>
                </Frame>
                <Text
                    @sizing=ItemSize::Percent(100.0)
                    string={label}
                    font_size=FONT_BODY
                    color={label_color}
                    align=TextAlign::Start
                />
            </CenteredRow>
        </Frame>
    }
}

pub fn checkbox_checked(document: &Document, checkbox: NodeId) -> bool {
    unstyled::toggle_checked(document, checkbox).get()
}

fn box_fill(theme: &Theme, checked: bool, hovered: bool) -> Color32 {
    match (checked, hovered) {
        (true, false) => theme.accent,
        (true, true) => theme.accent_hover,
        (false, false) => theme.surface_raised,
        (false, true) => theme.pressed,
    }
}

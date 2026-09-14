use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::color::Color32;

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{clone, create_memo, Callback, CenteredRow, Frame, ItemSize, Prop, Spacer};
use crate::styled::theme::{use_theme, Theme, BORDER_WIDTH, RADIUS};
use crate::unstyled;
use crate::unstyled::{Toggle, ToggleHandle};

const WIDTH: f32 = 42.0;
const HEIGHT: f32 = 24.0;
const KNOB_SIZE: f32 = 18.0;
const PADDING: f32 = 3.0;
const TRACK_RADIUS: u8 = 12;
const KNOB_RADIUS: u8 = 9;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 4.0;

#[component]
pub fn Switch(
    on: Prop<bool>,
    #[prop(default = String::new())] label: Prop<String>,
    on_change: Callback<bool>,
) -> NodeId {
    let accessibility = label.map(|label| {
        let mut node = Node::new(Role::Switch);
        if !label.is_empty() {
            node.set_label(label);
        }
        node
    });
    view! {
        <Toggle checked={on} accessibility on_change={move |on| on_change.call(on)}>
            {move |handle: ToggleHandle| {
                view! { <SwitchTrack handle /> }
            }}
        </Toggle>
    }
}

#[component]
fn SwitchTrack(handle: ToggleHandle) -> NodeId {
    let ToggleHandle {
        checked,
        hovered,
        focused,
        ..
    } = handle;
    let theme = use_theme();
    let before_percent = create_memo(clone!(checked -> move || before_size(checked.get())));
    let after_percent = create_memo(clone!(checked -> move || after_size(checked.get())));
    let track_color = create_memo(
        clone!(theme -> move || track_fill(&theme.get(), checked.get(), hovered.get())),
    );

    view! {
        <Frame outline={theme.pick(|theme| theme.accent)} outline_width=FOCUS_RING_WIDTH radius=RADIUS outline_offset=FOCUS_RING_OFFSET outline_visible={focused}>
            <Frame
                width=WIDTH
                height=HEIGHT
                color={track_color}
                outline={theme.pick(control_outline)}
                outline_width=BORDER_WIDTH
                outline_visible={theme.pick(|theme| theme.control_outline.is_some())}
                radius=TRACK_RADIUS
                padding_horizontal=PADDING
                padding_vertical=PADDING
            >
                <CenteredRow spacing=0.0>
                    <Spacer @sizing={before_percent} />
                    <Frame
                        width=KNOB_SIZE
                        height=KNOB_SIZE
                        color={theme.pick(|theme| theme.knob)}
                        outline={theme.pick(control_outline)}
                        outline_width=BORDER_WIDTH
                        outline_visible={theme.pick(|theme| theme.control_outline.is_some())}
                        radius=KNOB_RADIUS
                    />
                    <Spacer @sizing={after_percent} />
                </CenteredRow>
            </Frame>
        </Frame>
    }
}

pub fn switch_on(document: &Document, switch: NodeId) -> bool {
    unstyled::toggle_checked(document, switch).get()
}

fn control_outline(theme: &Theme) -> Color32 {
    theme.control_outline.unwrap_or(Color32::TRANSPARENT)
}

fn before_size(on: bool) -> ItemSize {
    if on {
        ItemSize::Percent(100.0)
    } else {
        ItemSize::Percent(0.0)
    }
}

fn after_size(on: bool) -> ItemSize {
    if on {
        ItemSize::Percent(0.0)
    } else {
        ItemSize::Percent(100.0)
    }
}

fn track_fill(theme: &Theme, on: bool, hovered: bool) -> Color32 {
    match (on, hovered) {
        (true, false) => theme.accent,
        (true, true) => theme.accent_hover,
        (false, false) => theme.surface_raised,
        (false, true) => theme.pressed,
    }
}

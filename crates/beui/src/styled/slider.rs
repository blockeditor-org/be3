use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::color::Color32;

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{Callback, CenteredRow, Frame, ItemSize, Prop, clone, create_memo};
use crate::styled::theme::{BORDER_WIDTH, RADIUS, Theme, use_theme};
use crate::unstyled;
use crate::unstyled::SliderHandle;

const HEIGHT: f32 = 20.0;
const TRACK_HEIGHT: f32 = 6.0;
const TRACK_RADIUS: u8 = 3;
const KNOB_SIZE: f32 = 16.0;
const KNOB_RADIUS: u8 = 8;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 3.0;

#[component]
pub fn Slider(
    value: Prop<f32>,
    #[prop(default = String::new())] label: Prop<String>,
    on_change: Callback<f32>,
) -> NodeId {
    let accessibility = label.map(|label| {
        let mut node = Node::new(Role::Slider);
        if !label.is_empty() {
            node.set_label(label);
        }
        node
    });
    view! {
        <unstyled::Slider value accessibility on_change={move |value| on_change.call(value)}>
            {move |handle: SliderHandle| {
                view! { <SliderTrack handle /> }
            }}
        </unstyled::Slider>
    }
}

#[component]
fn SliderTrack(handle: SliderHandle) -> NodeId {
    let SliderHandle {
        value,
        dragging,
        focused,
    } = handle;
    let theme = use_theme();
    let filled_percent = create_memo(clone!(value -> move || filled_size(value.get())));
    let rest_percent = create_memo(move || rest_size(value.get()));
    let knob_color =
        create_memo(clone!(theme -> move || knob_fill_color(&theme.get(), dragging.get())));

    view! {
        <Frame height=HEIGHT outline={theme.pick(|theme| theme.accent)} outline_width=FOCUS_RING_WIDTH radius=RADIUS outline_offset=FOCUS_RING_OFFSET outline_visible={focused}>
            <CenteredRow spacing=0.0>
                <Frame @sizing={filled_percent} height=TRACK_HEIGHT color={theme.pick(|theme| theme.accent)} radius=TRACK_RADIUS />
                <Frame
                    width=KNOB_SIZE
                    height=KNOB_SIZE
                    color={knob_color}
                    outline={theme.pick(|theme| theme.control_outline.unwrap_or(Color32::TRANSPARENT))}
                    outline_width=BORDER_WIDTH
                    outline_visible={theme.pick(|theme| theme.control_outline.is_some())}
                    radius=KNOB_RADIUS
                />
                <Frame @sizing={rest_percent} height=TRACK_HEIGHT color={theme.pick(|theme| theme.track)} radius=TRACK_RADIUS />
            </CenteredRow>
        </Frame>
    }
}

pub fn slider_value(document: &Document, slider: NodeId) -> f32 {
    unstyled::slider_value(document, slider).get()
}

fn filled_size(value: f32) -> ItemSize {
    ItemSize::Percent(value * 100.0)
}

fn rest_size(value: f32) -> ItemSize {
    ItemSize::Percent((1.0 - value) * 100.0)
}

fn knob_fill_color(theme: &Theme, dragging: bool) -> Color32 {
    if dragging {
        theme.accent_hover
    } else {
        theme.knob
    }
}

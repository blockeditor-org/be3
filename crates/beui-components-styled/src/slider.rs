use crate::frame::{Border, ColorRole, Frame, Radius, focus_border};
use beui_macros::{component, view};

use beui_core::color::Color32;

use crate::theme::{ThemeStore, knob_border, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{SliderHandle, SliderScale};
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const HEIGHT: f32 = 20.0;
const TRACK_HEIGHT: f32 = 6.0;
const TRACK_RADIUS: f32 = 3.0;
const KNOB_SIZE: f32 = 16.0;
const KNOB_RADIUS: f32 = 8.0;
const FOCUS_RING_OFFSET: f32 = 3.0;

#[component]
pub fn Slider(
    value: Binding<f32>,
    #[prop(default = 0.0)] min: f32,
    #[prop(default = 1.0)] max: f32,
    #[prop(default = SliderScale::Linear)] scale: SliderScale,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    on_drag_change: Callback<bool>,
) -> NodeId {
    view! {
        <unstyled::Slider
            value
            min
            max
            scale
            thumb=KNOB_SIZE
            disabled
            label
            on_drag_change={on_drag_change}
        >|handle: SliderHandle|
            <SliderTrack handle />
        </unstyled::Slider>
    }
}

#[component]
fn SliderTrack(handle: SliderHandle) -> NodeId {
    let SliderHandle {
        fraction,
        dragging,
        focused,
        disabled,
        ..
    } = handle;
    let theme = use_theme();
    let filled_percent = create_memo(move || filled_size(fraction.get()));
    let rest_percent = create_memo(move || rest_size(fraction.get()));
    let knob_color = create_memo(move || knob_fill_color(&theme, dragging.get(), disabled.get()));
    let filled_color = create_memo(move || match disabled.get() {
        true => theme.track.get(),
        false => theme.accent.get(),
    });

    view! {
        <Frame
            height=HEIGHT
            border={focus_border(focused, Border::FOCUS.offset(FOCUS_RING_OFFSET))}
            radius=Radius::Md
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=0.0>
                <Frame
                    @sizing={filled_percent}
                    height=TRACK_HEIGHT
                    fill={ColorRole::custom(filled_color)}
                    radius={Radius::custom(TRACK_RADIUS)}
                />
                <Frame
                    width=KNOB_SIZE
                    height=KNOB_SIZE
                    fill={ColorRole::custom(knob_color)}
                    border={knob_border(&theme)}
                    radius={Radius::custom(KNOB_RADIUS)}
                />
                <Frame
                    @sizing={rest_percent}
                    height=TRACK_HEIGHT
                    fill=ColorRole::Track
                    radius={Radius::custom(TRACK_RADIUS)}
                />
            </List>
        </Frame>
    }
}

pub fn slider_value(tree: &impl Inspect, slider: NodeId) -> f32 {
    unstyled::slider_value(tree, slider).get()
}

fn filled_size(value: f32) -> ItemSize {
    ItemSize::Percent(value * 100.0)
}

fn rest_size(value: f32) -> ItemSize {
    ItemSize::Percent((1.0 - value) * 100.0)
}

fn knob_fill_color(theme: &ThemeStore, dragging: bool, disabled: bool) -> Color32 {
    match (disabled, dragging) {
        (true, _) => theme.surface_raised.get(),
        (false, true) => theme.accent_hover.get(),
        (false, false) => theme.knob.get(),
    }
}

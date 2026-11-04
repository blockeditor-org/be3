use crate::frame::{Border, ColorRole, Frame, Radius, focus_border};
use beui_macros::{component, view};

use crate::list::{List, Spacer};
use crate::theme::{knob_border, use_theme};
use crate::tokens::{Size, Space, Stroke};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{SliderHandle, SliderScale};
use beui_view::primitives::Layers;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::{Align, Align2, Direction, ItemSize};

const HEIGHT: Size = Size::Sm;
const TRACK_HEIGHT: Size = Size::Track;
const KNOB_SIZE: Size = Size::Xs;
const MARK_WIDTH: Size = Size::Custom(2.0);

#[component]
pub fn Slider(
    #[prop(default = ComponentRef::new())] api: ComponentRef<crate::SliderApi>,

    value: Binding<f32>,
    #[prop(default = 0.0)] min: f32,
    #[prop(default = 1.0)] max: f32,
    #[prop(default = SliderScale::Linear)] scale: SliderScale,
    #[prop(default = None)] mark: Option<f32>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    on_drag_change: Callback<bool>,
) -> NodeId {
    view! {
        <unstyled::Slider
            api
            value
            min
            max
            scale
            thumb={KNOB_SIZE.px()}
            disabled
            label
            on_drag_change={on_drag_change}
        >|handle: SliderHandle|
            <SliderTrack handle mark={mark.map(|mark| scale.fraction_of(mark, min, max))} />
        </unstyled::Slider>
    }
}

#[component]
fn SliderTrack(handle: SliderHandle, mark: Prop<Option<f32>>) -> NodeId {
    let mark = mark.get_untracked();
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
    let knob_color = create_memo(move || knob_fill_color(dragging.get(), disabled.get()));
    let filled_color = create_memo(move || match disabled.get() {
        true => ColorRole::Track,
        false => ColorRole::Accent,
    });

    view! {
        <Frame
            height=HEIGHT
            border={focus_border(focused, Border::FOCUS.offset(Stroke::Focus))}
            radius=Radius::Md
        >
            <Layers>
                <Show condition={mark.is_some()}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::None>
                        <Spacer @sizing={filled_size(mark.unwrap_or_default())} />
                        <Frame width=KNOB_SIZE height=KNOB_SIZE align=Align2::CENTER>
                            <Frame
                                width=MARK_WIDTH
                                height=KNOB_SIZE
                                fill=ColorRole::TextMuted
                                radius=Radius::Full
                            />
                        </Frame>
                        <Spacer @sizing={rest_size(mark.unwrap_or_default())} />
                    </List>
                </Show>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::None>
                    <Frame
                        @sizing={filled_percent}
                        height=TRACK_HEIGHT
                        fill={filled_color}
                        radius=Radius::Full
                    />
                    <Frame
                        width=KNOB_SIZE
                        height=KNOB_SIZE
                        fill={knob_color}
                        border={knob_border(&theme)}
                        radius=Radius::Full
                    />
                    <Frame
                        @sizing={rest_percent}
                        height=TRACK_HEIGHT
                        fill=ColorRole::Track
                        radius=Radius::Full
                    />
                </List>
            </Layers>
        </Frame>
    }
}

fn filled_size(value: f32) -> ItemSize {
    ItemSize::Percent(value * 100.0)
}

fn rest_size(value: f32) -> ItemSize {
    ItemSize::Percent((1.0 - value) * 100.0)
}

fn knob_fill_color(dragging: bool, disabled: bool) -> ColorRole {
    match (disabled, dragging) {
        (true, _) => ColorRole::Control,
        (false, true) => ColorRole::AccentHover,
        (false, false) => ColorRole::Knob,
    }
}

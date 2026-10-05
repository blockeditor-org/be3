use accesskit::Role;
use beui_macros::{component, view};

use crate::focus_ring::FocusRing;
use beui_core::color::Color32;

use crate::theme::{BORDER_WIDTH, ThemeStore, control_outline, control_outline_visible, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{Toggle, ToggleHandle};
use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_view::reactive::{Align, Callback, Frame, Prop, clone, create_memo};

const WIDTH: f32 = 42.0;
const HEIGHT: f32 = 24.0;
const KNOB_SIZE: f32 = 18.0;
const PADDING: f32 = 3.0;
const TRACK_RADIUS: u8 = 12;
const KNOB_RADIUS: u8 = 9;
const FOCUS_RING_OFFSET: f32 = 4.0;

#[component]
pub fn Switch(
    on: Prop<bool>,
    #[prop(default = String::new())] label: Prop<String>,
    on_change: Callback<bool>,
) -> NodeId {
    view! {
        <Toggle checked={on} label role=Role::Switch on_change={move |on| on_change.call(on)}>
            {move |handle: ToggleHandle| {
                view! {
                    <SwitchTrack handle />
                }
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
    let knob_align = create_memo(clone!(checked -> move || knob_align(checked.get())));
    let track_color =
        create_memo(clone!(theme -> move || track_fill(&theme, checked.get(), hovered.get())));

    view! {
        <FocusRing focused offset=FOCUS_RING_OFFSET>
            <Frame
                width=WIDTH
                height=HEIGHT
                color={track_color}
                outline={control_outline(&theme)}
                outline_width=BORDER_WIDTH
                outline_visible={control_outline_visible(&theme)}
                radius=TRACK_RADIUS
                padding_horizontal=PADDING
                padding_vertical=PADDING
                align_horizontal={knob_align}
                align_vertical=Align::Center
            >
                <Frame
                    width=KNOB_SIZE
                    height=KNOB_SIZE
                    color={theme.knob.clone()}
                    outline={control_outline(&theme)}
                    outline_width=BORDER_WIDTH
                    outline_visible={control_outline_visible(&theme)}
                    radius=KNOB_RADIUS
                />
            </Frame>
        </FocusRing>
    }
}

pub fn switch_on(document: &Document, switch: NodeId) -> bool {
    unstyled::toggle_checked(document, switch).get()
}

fn knob_align(on: bool) -> Align {
    if on { Align::End } else { Align::Start }
}

fn track_fill(theme: &ThemeStore, on: bool, hovered: bool) -> Color32 {
    match (on, hovered) {
        (true, false) => theme.accent.get(),
        (true, true) => theme.accent_hover.get(),
        (false, false) => theme.surface_raised.get(),
        (false, true) => theme.pressed.get(),
    }
}

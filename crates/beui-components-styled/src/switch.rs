use accesskit::Role;
use beui_macros::{component, view};

use crate::focus_ring::FocusRing;
use beui_core::color::Color32;

use crate::theme::{ThemeStore, control_outline, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{Toggle, ToggleHandle};
use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const WIDTH: f32 = 42.0;
const HEIGHT: f32 = 24.0;
const KNOB_SIZE: f32 = 18.0;
const PADDING: f32 = 3.0;
const TRACK_RADIUS: f32 = 12.0;
const KNOB_RADIUS: f32 = 9.0;
const FOCUS_RING_OFFSET: f32 = 4.0;

#[component]
pub fn Switch(
    on: Prop<bool>,
    #[prop(default = false)] indeterminate: Prop<bool>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    on_change: Callback<bool>,
) -> NodeId {
    view! {
        <Toggle
            checked={on}
            mixed={indeterminate}
            label
            disabled
            role=Role::Switch
            on_change={on_change}
        >
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
        mixed,
        hovered,
        focused,
        disabled,
        ..
    } = handle;
    let theme = use_theme();
    let knob_align =
        create_memo(move || Align2::new(knob_align(checked.get(), mixed.get()), Align::Center));
    let track_color = create_memo(move || {
        track_fill(
            &theme,
            disabled.get(),
            checked.get() && !mixed.get(),
            hovered.get(),
        )
    });

    view! {
        <FocusRing focused offset=FOCUS_RING_OFFSET>
            <Frame
                width=WIDTH
                height=HEIGHT
                fill=track_color
                outline={control_outline(&theme)}
                radius=TRACK_RADIUS
                padding=PADDING
                align=knob_align
            >
                <Frame
                    width=KNOB_SIZE
                    height=KNOB_SIZE
                    fill={theme.knob}
                    outline={control_outline(&theme)}
                    radius=KNOB_RADIUS
                />
            </Frame>
        </FocusRing>
    }
}

pub fn switch_on(document: &Document, switch: NodeId) -> bool {
    unstyled::toggle_checked(document, switch).get()
}

fn knob_align(on: bool, mixed: bool) -> Align {
    match (mixed, on) {
        (true, _) => Align::Center,
        (false, true) => Align::End,
        (false, false) => Align::Start,
    }
}

fn track_fill(theme: &ThemeStore, disabled: bool, on: bool, hovered: bool) -> Color32 {
    if disabled {
        return match on {
            true => theme.accent_soft.get(),
            false => theme.surface.get(),
        };
    }
    match (on, hovered) {
        (true, false) => theme.accent.get(),
        (true, true) => theme.accent_hover.get(),
        (false, false) => theme.surface_raised.get(),
        (false, true) => theme.pressed.get(),
    }
}

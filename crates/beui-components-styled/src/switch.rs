use crate::frame::{ColorRole, Frame, Radius};
use accesskit::Role;
use beui_macros::{component, view};

use crate::focus_ring::FocusRing;
use crate::theme::{control_border, use_theme};
use crate::tokens::{Size, Space, Stroke};
use beui_components_unstyled::{Toggle, ToggleHandle};
use beui_core::node::NodeId;
use beui_view::reactive::*;

const WIDTH: Size = Size::Custom(42.0);
const HEIGHT: Size = Size::Md;
const KNOB_SIZE: Size = Size::Custom(18.0);
const PADDING: Space = Space::Custom(3.0);

#[component]
pub fn Switch(
    #[prop(default = ComponentRef::new())] api: ComponentRef<crate::ToggleApi>,

    on: Binding<bool>,
    #[prop(default = false)] indeterminate: Prop<bool>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
) -> NodeId {
    view! {
        <Toggle
            api
            checked={on}
            mixed={indeterminate}
            label
            disabled
            role=Role::Switch
        >|handle: ToggleHandle|
            <SwitchTrack handle />
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
            disabled.get(),
            checked.get() && !mixed.get(),
            hovered.get(),
        )
    });

    view! {
        <FocusRing focused radius=Radius::Full offset=Stroke::Focus>
            <Frame
                width=WIDTH
                height=HEIGHT
                fill={track_color}
                border={control_border(&theme)}
                radius=Radius::Full
                padding=PADDING
                align=knob_align
            >
                <Frame
                    width=KNOB_SIZE
                    height=KNOB_SIZE
                    fill=ColorRole::Knob
                    border={control_border(&theme)}
                    radius=Radius::Full
                />
            </Frame>
        </FocusRing>
    }
}

fn knob_align(on: bool, mixed: bool) -> Align {
    match (mixed, on) {
        (true, _) => Align::Center,
        (false, true) => Align::End,
        (false, false) => Align::Start,
    }
}

fn track_fill(disabled: bool, on: bool, hovered: bool) -> ColorRole {
    if disabled {
        return match on {
            true => ColorRole::AccentSoft,
            false => ColorRole::Surface,
        };
    }
    match (on, hovered) {
        (true, false) => ColorRole::Accent,
        (true, true) => ColorRole::AccentHover,
        (false, false) => ColorRole::SurfaceRaised,
        (false, true) => ColorRole::Pressed,
    }
}

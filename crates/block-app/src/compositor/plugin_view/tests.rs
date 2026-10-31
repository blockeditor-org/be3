use super::*;
use block_app_plugin_host::HostContent;
use block_plugin_api::{ChildLayer, ChildMode, HostWindowId, TopBar};

pub(super) fn child(clip: Rect) -> HostChild {
    HostChild {
        child: ChildId(7),
        frame_owner: false,
        own_frame: false,
        top_bar: TopBar::Hidden,
        content: HostContent::Window(HostWindowId(1)),
        rect: clip,
        view_rect: clip,
        occluders: Vec::new(),
        clip,
        layer: ChildLayer::Above,
        mode: ChildMode::Active,
        intrinsic: None,
        rotation: 0.0,
        opacity: 1.0,
        corner_radius: 0.0,
        dialog: None,
        anchored: None,
        keyboard: false,
        sheet: false,
        anchor: None,
    }
}

pub(super) fn placed(children: Vec<HostChild>) -> PlacedView {
    PlacedView {
        error: None,
        rect: Rect::ZERO,
        clip: Rect::ZERO,
        loading: false,
        base: Vec::new(),
        floating: Vec::new(),
        floating_rects: Vec::new(),
        scrims: Vec::new(),
        drawing: Vec::new(),
        claims: Vec::new(),
        held: None,
        drawn: None,
        children,
        anchored: Vec::new(),
        blocks_owner: false,
        holes: Vec::new(),
        cursor: None,
        ime: None,
        handles_back: false,
        wants_keyboard: false,
        keyboard_toggles: None,
        intercepted_keys: Vec::new(),
        pastes: Vec::new(),
        clipboard_keys: false,
        every_sample: false,
        work_area: None,
    }
}

mod a_child_moving_into_another_view_of_its_instance_keeps_its_node_and_follows_it;
mod a_child_moving_out_of_a_view_during_layout_leaves_no_removed_node_to_paint;
mod a_pan_drag_asked_for_late_pans_from_the_press;

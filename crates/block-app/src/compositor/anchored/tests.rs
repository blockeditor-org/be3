use super::*;
use beui::{PointerButton, PointerEvent, PointerId, pos2, vec2};

fn shown(mount: u64, id: u64, opened: u64, rect: Rect) -> Shown {
    Shown {
        mount,
        anchored: HostAnchored {
            id: ViewId(id),
            parent: None,
            opened,
            screen: block_plugin_api::ScreenId::opened(id),
            rect,
            bounds: Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0)),
            painted: vec![rect],
            floating: vec![rect],
            scrims: Vec::new(),
            surface: 1,
            pixels_per_point: 1.0,
            dismiss_on_outside_press: true,
            trigger: None,
            keyboard: false,
            blocks_owner: false,
            edge: false,
        },
    }
}

fn mouse(phase: PointerPhase, x: f32, y: f32) -> Forwarded {
    Forwarded::Pointer {
        event: PointerEvent {
            pointer: PointerId::Mouse,
            window: WindowId::MAIN,
            phase,
            pos: pos2(x, y),
            modifiers: Default::default(),
            clicks: 1,
            force: None,
            pen: None,
            predicted: None,
            time: std::time::Instant::now(),
            source: 0,
        },
        over: None,
    }
}

fn input(events: Vec<Forwarded>) -> ForwardedInput {
    ForwardedInput {
        events,
        rect: Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0)),
        hovered: true,
        focused: false,
        pointer: None,
        modifiers: Default::default(),
    }
}

fn targets(runs: &[(Target, Vec<Forwarded>, bool)]) -> Vec<Target> {
    runs.iter().map(|(target, ..)| *target).collect()
}

mod a_child_in_an_anchored_view_goes_with_its_view_before_what_the_view_owns;
mod a_child_views_anchor_bounds_never_exceed_its_parents;
mod a_press_on_an_anchored_view_keeps_its_drag_until_it_is_let_go;
mod a_view_a_child_in_an_anchored_view_anchors_takes_presses_over_that_child;
mod an_anchored_view_hears_the_pointer_leave_it;
mod an_anchored_view_hears_the_pointer_leave_the_window;
mod the_anchored_view_opened_last_takes_a_press_where_they_overlap;
mod the_host_window_follows_a_window_that_becomes_real;

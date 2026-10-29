use super::*;
use beui::{PointerButton, PointerEvent, PointerId, pos2, vec2};
use block_plugin_api::LayerParent;

fn shown(mount: u64, id: u64, opened: u64, rect: Rect) -> Shown {
    Shown {
        mount,
        layer: HostLayer {
            id: LayerId(id),
            parent: LayerParent::Region,
            opened,
            rect,
            bounds: Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0)),
            painted: vec![rect],
            floating: vec![rect],
            scrims: Vec::new(),
            surface: 1,
            pixels_per_point: 1.0,
            dismiss_on_outside_press: true,
            keyboard: false,
            blocks_owner: false,
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

mod a_child_region_s_layer_bounds_never_exceed_its_parent_s;
mod a_layer_a_child_in_a_layer_opens_takes_presses_over_that_child;
mod a_press_on_a_layer_keeps_its_drag_until_it_is_let_go;
mod the_layer_opened_last_takes_a_press_where_layers_overlap;

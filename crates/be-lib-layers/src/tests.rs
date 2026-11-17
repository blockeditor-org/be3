use be_lib_input_router::{Routed, Router};
use be_lib_ui_types::input::{Input, PointerButton};

use super::*;

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::from_min_max(pos2(x, y), pos2(x + width, y + height))
}

/// A surface of `size` pixels at two pixels per point, placed with its top left at `at`.
fn layer(id: u64, at: Pos2, size: (u32, u32)) -> Layer {
    Layer {
        id: LayerId(id),
        surface: Surface::Screen {
            instance: id as u32,
            surface: 0,
        },
        size,
        transform: Affine::from_axes(vec2(0.5, 0.0), vec2(0.0, 0.5), at),
        clip: Rect::from_min_max(Pos2::ZERO, pos2(size.0 as f32, size.1 as f32)),
        corner_radius: 0.0,
        opacity: 1.0,
        shown: true,
        input: LayerInput::default(),
    }
}

fn window(layers: Vec<Layer>) -> WindowLayers {
    WindowLayers {
        window: None,
        layers,
    }
}

fn close(one: Pos2, other: Pos2) -> bool {
    (one - other).length() < 1e-4
}

mod a_layer_raised_over_another_damages_only_where_they_overlap;
mod a_layer_that_captures_presses_hands_those_outside_it_to_its_placer;
mod a_point_lands_on_the_topmost_layer_that_takes_it_there;
mod a_transform_and_its_inverse_take_a_point_there_and_back;
mod damage_is_each_surfaces_own_through_its_layer_and_the_bounds_of_layers_that_changed;
mod raising_a_layer_past_several_damages_only_those_it_overlaps;
mod the_clip_rounds_only_the_surfaces_own_corners;

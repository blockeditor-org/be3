use super::*;

mod an_unchanged_paint_list_has_no_damage;
mod damage_covers_a_shape_inserted_between_unchanged_neighbours;
mod damage_covers_the_old_and_new_bounds_of_a_moved_shape;
mod damage_is_clipped_to_the_clip_rectangle_of_the_shape;

use crate::color::Color32;
use crate::geometry::{Rect, pos2};

fn rect(left: f32, top: f32, right: f32, bottom: f32) -> Rect {
    Rect::from_min_max(pos2(left, top), pos2(right, bottom))
}

fn filled(bounds: Rect) -> Shape {
    clipped(bounds, Rect::EVERYTHING)
}

fn clipped(bounds: Rect, clip: Rect) -> Shape {
    Shape::Rect {
        rect: bounds,
        corner_radius: 0.0,
        stroke_width: 0.0,
        color: Color32::WHITE,
        clip,
    }
}

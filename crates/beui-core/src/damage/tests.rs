use super::*;

mod damage_far_apart_stays_in_separate_regions;
mod damage_that_overlaps_what_is_held_merges_into_it;
mod damaging_everything_covers_the_whole_viewport;
mod more_damage_than_there_are_regions_merges_the_cheapest_pair;
mod taking_the_damage_clips_it_and_starts_again;
mod the_bounds_of_a_shape_stop_at_its_clip_rectangle;

use crate::color::Color32;
use crate::geometry::pos2;

fn rect(left: f32, top: f32, right: f32, bottom: f32) -> Rect {
    Rect::from_min_max(pos2(left, top), pos2(right, bottom))
}

fn clipped(bounds: Rect, clip: Rect) -> Shape {
    Shape::Rect {
        rect: bounds,
        corner_radius: 0.0,
        stroke_width: 0.0,
        color: Color32::WHITE,
        rotation: crate::geometry::Rotation::NONE,
        clip,
    }
}

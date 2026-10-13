use super::*;

mod damage_far_apart_stays_in_separate_regions;
mod damage_that_lines_up_with_what_is_held_joins_it;
mod damage_that_overlaps_what_is_held_adds_only_what_it_does_not_cover;
mod damaging_everything_covers_the_whole_viewport;
mod merging_regions_counts_the_damage_a_merge_swallows;
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
        corner_radius: 0.0.into(),
        stroke_width: 0.0,
        color: Color32::WHITE,
        rotation: crate::geometry::Rotation::NONE,
        clip,
    }
}

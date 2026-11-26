use be_lib_geometry::{Pos2, Rect, Vec2, pos2, vec2};
use beui_tree::reactive::RootScope;

use super::*;
use crate::input::Modifiers;

mod a_closed_point_floats_over_its_places;
mod a_custom_site_chooses_the_drop_its_place_gives_for_the_point;
mod a_custom_sites_stops_are_the_places_a_keyboard_steps_between;
mod a_drag_floats_beyond_the_reach_of_its_near_places;
mod a_far_place_takes_over_where_every_near_place_is_closed;
mod a_fast_release_lands_where_its_path_leads;
mod a_following_place_draws_the_item_under_the_pointer_along_its_line;
mod a_free_drag_stays_inside_its_area;
mod a_held_modifier_chooses_among_its_own_places;
mod a_keyboard_step_takes_the_nearest_place_that_way;
mod a_longer_throw_carries_a_release_further;
mod a_shaped_place_draws_the_item_under_the_pointer_at_its_size;
mod a_zone_inside_a_held_zone_takes_the_pointer_that_enters_it;
mod a_zone_left_lets_go_of_its_place;
mod a_zoned_place_is_chosen_while_the_pointer_is_in_its_zone;
mod an_edge_place_takes_the_corner_up_to_its_diagonal;
mod an_unbounded_free_drag_goes_past_its_areas_edges;
mod closest_snaps_to_the_nearest_rest_and_holds_it_until_another_is_clearly_nearer;
mod moving_after_a_neighbour_names_the_place_by_the_row_before;

/// A node at the origin at scale 1, and how the board would place it.
fn origin(_root: &RootScope) -> NodeRef {
    let node = NodeRef::new();
    node.fill(NodeId::new(1, 0));
    node
}

fn placed(_: NodeId) -> Option<(Rect, f32)> {
    Some((Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 1000.0)), 1.0))
}

/// A 20-point item held by its centre at `pos`.
fn at(pos: Pos2) -> DragPoint {
    DragPoint {
        item: Rect::from_min_size(pos2(-10.0, -10.0), vec2(20.0, 20.0)),
        ..DragPoint::at(pos, Modifiers::NONE)
    }
}

fn slot(node: NodeRef, drop: u32, x: f32) -> Candidate<u32> {
    Candidate::new(
        drop,
        Rect::from_min_size(pos2(x, 0.0), vec2(20.0, 20.0)),
        node,
    )
}

fn frame(spec: &DragSpec<u32>, point: DragPoint, last: &DragFrame) -> Evaluated<u32> {
    evaluate(
        spec,
        &Evaluation {
            point,
            velocity: Vec2::ZERO,
            released: false,
            placed: &placed,
        },
        last,
    )
}

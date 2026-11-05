use super::*;
use beui::{pos2, vec2};

mod a_deliberate_wall_counts_pushes_only_after_the_pointer_rests_against_it;
mod a_finger_landing_by_a_wall_swipes_it_and_says_how_far_it_reached;
mod a_wall_between_displays_stops_the_pointer_until_it_opens;
mod pushing_past_a_wall_counts_only_after_the_pointer_arrives;
mod the_pointer_is_near_an_open_edge_inside_its_keep;

const MS: u64 = 1_000;

fn display() -> Rect {
    Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0))
}

fn below() -> Rect {
    Rect::from_min_size(pos2(0.0, 600.0), vec2(800.0, 600.0))
}

fn bottom(wall: bool, deliberate: bool) -> ScreenEdge {
    ScreenEdge {
        id: 7,
        display: display(),
        side: Side::Bottom,
        start: 0.0,
        end: 800.0,
        wall,
        deliberate,
        keep: None,
    }
}

fn edges(edge: ScreenEdge) -> Edges {
    let mut edges = Edges::default();
    edges.set(vec![edge]);
    edges
}

fn pushed(edges: &Edges) -> f32 {
    edges.states()[0].pushed
}

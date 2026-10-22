use std::time::{Duration, Instant};

use winit::event::TouchPhase;

use super::*;

mod a_lifted_trackpad_scroll_ends_and_the_systems_momentum_after_it_is_dropped;
mod a_new_scroll_soon_after_a_lift_scrolls_where_the_system_adds_no_momentum;

fn phases(wheel: &mut Wheel, start: Instant, events: &[(u64, TouchPhase)]) -> Vec<Wheeled> {
    events
        .iter()
        .map(|(millis, phase)| wheel.event(*phase, start + Duration::from_millis(*millis)))
        .collect()
}

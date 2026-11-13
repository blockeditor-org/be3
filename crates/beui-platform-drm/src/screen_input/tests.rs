use super::*;
use beui::{pos2, vec2};

mod a_region_hears_whether_the_pointer_is_in_it;
mod a_region_that_captures_touch_takes_fingers_landing_in_it;
mod a_wall_between_displays_stops_the_pointer_until_it_goes;
mod a_wall_stops_the_pointer_and_says_how_far_it_was_pushed_on;

fn display() -> Rect {
    Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0))
}

fn below() -> Rect {
    Rect::from_min_size(pos2(0.0, 600.0), vec2(800.0, 600.0))
}

fn bottom() -> Wall {
    Wall {
        id: 7,
        display: display(),
        side: Side::Bottom,
        start: 0.0,
        end: 800.0,
    }
}

/// A strip along the foot of the display.
fn foot(captures_touch: bool) -> Region {
    Region {
        id: 9,
        rect: Rect::from_min_size(pos2(0.0, 540.0), vec2(800.0, 60.0)),
        captures_touch,
    }
}

fn input(walls: Vec<Wall>, regions: Vec<Region>) -> Input {
    let mut input = Input::default();
    input.set(walls, regions);
    input
}

fn wall(input: &Input) -> WallState {
    input.state().walls[0].clone()
}

fn region(input: &Input) -> RegionState {
    input.state().regions[0].clone()
}

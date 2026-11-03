use beui::{pos2, vec2};

use super::*;

mod a_chosen_display_wins_over_every_hint;
mod a_pen_display_lands_on_its_own_screen_turned_with_it;
mod a_touchscreen_lands_on_the_display_its_size_matches;
mod an_integrated_touchscreen_lands_on_the_built_in_panel;
mod a_tablet_that_matches_no_display_spans_them_all;
mod the_udev_output_hint_names_the_display;

fn laptop() -> Screen {
    Screen {
        id: "laptop-panel".to_owned(),
        connector: "eDP-1".to_owned(),
        size: Some((310, 174)),
        rect: Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 720.0)),
        rotation: Rotation::Normal,
    }
}

fn pen_display(rotation: Rotation) -> Screen {
    Screen {
        id: "WAC|Cintiq 16|1234".to_owned(),
        connector: "HDMI-A-1".to_owned(),
        size: Some((344, 194)),
        rect: Rect::from_min_size(pos2(1280.0, 0.0), vec2(960.0, 540.0)),
        rotation,
    }
}

fn pointer(kind: Kind, size: Option<(f64, f64)>) -> Pointer {
    Pointer {
        kind,
        size,
        output: None,
    }
}

fn areas(screens: &[Screen]) -> Vec<(Rect, Rotation)> {
    screens
        .iter()
        .map(|screen| (screen.rect, screen.rotation))
        .collect()
}

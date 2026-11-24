use be_lib_geometry::{Rect, pos2, vec2};
use be_lib_layers::{Affine, Layer, LayerInput, Surface};
use be_lib_ui_types::{ImeArea, Keyboard};

use super::*;

mod a_press_or_escape_takes_back_an_active_child_and_a_release_does_not;
mod a_window_asks_for_what_the_screens_it_shows_ask_of_theirs;
mod an_ime_area_in_a_screen_lands_where_its_layer_shows_it;
mod anchored_views_hang_in_chains_from_their_editors_views;

/// A screen painted at `raster` pixels per point, shown a point to a point with its top left at
/// `origin` in the window.
fn shown_at(origin: (f32, f32), raster: f32) -> Layer {
    let points = 1.0 / raster;
    Layer {
        id: LayerId(1),
        surface: Surface::Screen {
            instance: 3,
            surface: 4,
        },
        size: (640, 400),
        transform: Affine::from_axes(
            vec2(points, 0.0),
            vec2(0.0, points),
            pos2(origin.0, origin.1),
        ),
        clip: Rect::from_min_max(pos2(0.0, 0.0), pos2(640.0, 400.0)),
        corner_radius: 0.0,
        opacity: 1.0,
        shown: true,
        input: LayerInput::default(),
    }
}

fn area(rect: Rect, cursor: Rect) -> ImeArea {
    ImeArea {
        rect,
        cursor,
        text: None,
        keyboard: Keyboard::Shown { asks: 0 },
    }
}

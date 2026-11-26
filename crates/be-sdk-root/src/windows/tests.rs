use be_lib_geometry::{Rect, pos2, vec2};
use be_lib_layers::{Affine, Layer, LayerInput};
use be_lib_ui_types::input::PointerButton;

use super::*;

mod a_focused_layer_that_went_loses_the_focus;
mod a_lock_the_platform_released_is_granted_again_only_once_asked_anew;
mod a_window_that_went_routes_by_its_last_layers_until_its_press_lifts;
mod an_intercepted_chord_goes_to_the_compositor_ahead_of_the_focus;
mod each_delivery_carries_the_time_its_input_was_sampled;
mod input_for_a_window_never_drawn_is_routed_against_no_layers;
mod the_focus_given_back_to_a_layer_that_went_goes_to_no_layer;
mod the_focus_is_the_layer_a_press_landed_on;
mod the_focus_moved_to_a_layer_and_back_tells_each_layer;
mod the_focused_layer_that_asks_holds_the_pointer_lock_until_it_stops_asking;
mod the_release_of_a_press_on_a_layer_that_went_reaches_nothing;
mod the_release_of_an_escape_that_closed_a_dialog_never_reaches_the_layer_the_focus_went_back_to;

/// The root's own layer, which takes intercepted keys.
const ROOT: LayerId = LayerId(0);

/// An app's screen, in the window's right half.
const SCREEN: LayerId = LayerId(1);

const SCREEN_SURFACE: Surface = Surface::Screen {
    instance: 3,
    surface: 4,
};

fn layer(id: LayerId, surface: Surface, x: f32) -> Layer {
    Layer {
        id,
        surface,
        size: (100, 100),
        transform: Affine::from_axes(vec2(1.0, 0.0), vec2(0.0, 1.0), pos2(x, 0.0)),
        clip: Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 100.0)),
        corner_radius: 0.0,
        opacity: 1.0,
        shown: true,
        input: LayerInput::default(),
    }
}

/// The host's one window: the root's surface, and an app's screen over its right half.
fn drawn() -> LayerList {
    LayerList {
        windows: vec![WindowLayers {
            window: None,
            layers: vec![
                layer(ROOT, Surface::Root(0), 0.0),
                layer(SCREEN, SCREEN_SURFACE, 100.0),
            ],
        }],
    }
}

fn button(x: f32, pressed: bool) -> Input {
    Input::PointerButton {
        pos: pos2(x, 50.0),
        button: PointerButton::Primary,
        pressed,
    }
}

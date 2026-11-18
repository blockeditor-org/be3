use be_lib_geometry::{Rect, pos2, vec2};
use be_lib_layers::{Affine, Layer, LayerInput};
use be_lib_ui_types::input::PointerButton;

use super::*;

mod a_window_that_went_routes_by_its_last_layers_until_its_press_lifts;
mod input_for_a_window_never_drawn_is_routed_against_no_layers;
mod the_focus_is_the_layer_a_press_landed_on;

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

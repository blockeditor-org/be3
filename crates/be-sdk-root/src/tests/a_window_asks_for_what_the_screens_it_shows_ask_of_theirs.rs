use be_lib_layers::{LayerList, WindowLayers};
use be_lib_ui_types::Input;

use super::*;

#[test]
fn a_window_asks_for_what_the_screens_it_shows_ask_of_theirs() {
    let mut windows = Windows::new(LayerId(0));
    let mut own = shown_at((0.0, 0.0), 1.0);
    own.id = LayerId(0);
    own.surface = Surface::Root(0);
    // The screen is 640 by 400 pixels at 2 a point, so 320 by 200 points, at (40, 20), over
    // the root's own surface.
    let screen = shown_at((40.0, 20.0), 2.0);
    windows.drew(LayerList {
        windows: vec![WindowLayers {
            window: None,
            layers: vec![own, screen.clone()],
        }],
    });
    let mut asked = WindowRequests {
        cursor: CursorIcon::Crosshair,
        every_sample: true,
        ..WindowRequests::default()
    };
    let requests_now = |windows: &Windows, asked: &WindowRequests| {
        requests(windows, None, |surface| {
            (*surface == screen.surface).then(|| asked.clone())
        })
    };

    assert_eq!(
        requests_now(&windows, &asked),
        Requests {
            cursor: CursorIcon::Default,
            pointer_locked: false,
            every_sample: true,
        },
        "the pointer is over nothing yet, but the screen shown wants every sample"
    );

    windows.route(None, vec![Input::PointerMoved(pos2(100.0, 100.0)).into()]);
    assert_eq!(
        requests_now(&windows, &asked).cursor,
        CursorIcon::Crosshair,
        "over the screen, the window shows the screen's cursor"
    );
    windows.route(None, vec![Input::PointerMoved(pos2(10.0, 10.0)).into()]);
    assert_eq!(
        requests_now(&windows, &asked).cursor,
        CursorIcon::Default,
        "over the root's own surface, which asks nothing, the default one"
    );

    asked.pointer_locked = true;
    windows.route(
        None,
        vec![
            Input::PointerButton {
                pos: pos2(100.0, 100.0),
                button: be_lib_ui_types::PointerButton::Primary,
                pressed: true,
            }
            .into(),
        ],
    );
    windows.lock(|layer| layer.id == screen.id);
    assert!(
        requests_now(&windows, &asked).pointer_locked,
        "the screen the press focused holds the lock it asks for"
    );

    asked.every_sample = false;
    assert!(
        !requests_now(&windows, &asked).every_sample,
        "no screen shown wants every sample"
    );
}

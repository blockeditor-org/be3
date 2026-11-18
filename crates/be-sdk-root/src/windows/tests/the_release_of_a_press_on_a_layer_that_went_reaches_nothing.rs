use be_lib_ui_types::input::{TouchId, TouchPhase};

use super::*;

/// A dialog of the root's own over the screen, which closes on a press.
const DIALOG: LayerId = LayerId(2);

#[test]
fn the_release_of_a_press_on_a_layer_that_went_reaches_nothing() {
    let mut windows = Windows::new(ROOT);
    let with_dialog = || {
        let mut list = drawn();
        list.windows[0]
            .layers
            .push(layer(DIALOG, Surface::Root(1), 100.0));
        list
    };
    windows.drew(with_dialog());
    let pressed = windows.route(None, vec![button(150.0, true).into()]);
    assert!(pressed.iter().any(|told| told.target == DIALOG));

    // The dialog closes on the press: the screen under it never heard the press, so it hears
    // nothing of its release either.
    windows.drew(drawn());
    let released = windows.route(None, vec![button(150.0, false).into()]);
    assert!(
        released.is_empty(),
        "the release of a press on a layer that went reaches nothing: {released:?}"
    );

    // A finger likewise.
    windows.drew(with_dialog());
    let touch = |phase| {
        Timed::from(Input::Touch {
            id: TouchId {
                device: 0,
                finger: 1,
            },
            phase,
            pos: pos2(150.0, 50.0),
            force: None,
        })
    };
    windows.route(None, vec![touch(TouchPhase::Start)]);
    windows.drew(drawn());
    let ended = windows.route(None, vec![touch(TouchPhase::End)]);
    assert!(
        ended.is_empty(),
        "the end of a touch on a layer that went reaches nothing: {ended:?}"
    );

    // Pressed afresh, the screen takes the press.
    let fresh = windows.route(None, vec![button(150.0, true).into()]);
    assert!(fresh.iter().any(|told| told.target == SCREEN));
}

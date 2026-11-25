use super::*;

#[test]
fn a_window_that_went_routes_by_its_last_layers_until_its_press_lifts() {
    let mut windows = Windows::new(ROOT);
    windows.drew(drawn());
    let pressed = windows.route(None, vec![button(150.0, true).into()]);
    let to_screen = |input| Delivery {
        target: SCREEN,
        surface: SCREEN_SURFACE,
        input,
        time: None,
        modifiers: Modifiers::NONE,
    };
    assert_eq!(
        pressed,
        vec![to_screen(Input::Focus(true)), to_screen(button(50.0, true))],
        "the press focuses the screen and lands on it, in its own coordinates"
    );

    windows.drew(LayerList::default());
    assert!(
        windows.routes(None),
        "the window went, but its press has yet to lift"
    );
    let released = windows.route(None, vec![button(150.0, false).into()]);
    assert_eq!(
        released,
        vec![
            to_screen(button(50.0, false)),
            to_screen(Input::Focus(false))
        ],
        "the release reaches what was pressed, by the layers the window had last, and then the \
         screen hears the window took its focus with it"
    );
    assert!(
        !windows.routes(None),
        "with nothing pressed on it, the window's router goes"
    );
}

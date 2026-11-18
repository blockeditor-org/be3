use be_lib_ui_types::input::{Key, Keystroke};

use super::*;

/// A dialog of the root's own, over the root's own surface.
const DIALOG: LayerId = LayerId(2);

#[test]
fn the_focus_given_back_to_a_layer_that_went_goes_to_no_layer() {
    let mut windows = Windows::new(ROOT);
    let with_dialog = || {
        let mut list = drawn();
        list.windows[0]
            .layers
            .push(layer(DIALOG, Surface::Root(1), 0.0));
        list
    };
    windows.drew(with_dialog());
    windows.route(
        None,
        vec![button(150.0, true).into(), button(150.0, false).into()],
    );
    let had = windows.focused(None).map(|layer| layer.id);
    windows.focus(None, Some(DIALOG));

    // While the dialog is open, the screen that had the focus goes.
    let mut list = with_dialog();
    list.windows[0].layers.retain(|layer| layer.id != SCREEN);
    windows.drew(list);
    assert!(
        windows.focus(None, had).iter().all(|told| told.target != SCREEN),
        "the screen that went hears nothing"
    );
    assert_eq!(
        windows.focused(None),
        None,
        "the focus given back to a layer that went goes to no layer"
    );

    // A screen drawn later with the same id is not focused without hearing it.
    windows.drew(drawn());
    assert_eq!(windows.focused(None), None);
    let key = windows.route(
        None,
        vec![
            Input::Key(Keystroke {
                key: Some(Key::A),
                pressed: true,
                ..Keystroke::default()
            })
            .into(),
        ],
    );
    assert!(key.is_empty(), "a key reaches no layer: {key:?}");
}

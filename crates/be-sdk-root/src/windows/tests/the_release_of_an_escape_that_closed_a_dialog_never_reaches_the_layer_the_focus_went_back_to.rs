use be_lib_ui_types::input::{Key, Keystroke};

use super::*;

/// A dialog of the root's own, over the root's own surface.
const DIALOG: LayerId = LayerId(2);

fn escape(pressed: bool) -> Timed {
    Input::Key(Keystroke {
        key: Some(Key::Escape),
        pressed,
        ..Keystroke::default()
    })
    .into()
}

#[test]
fn the_release_of_an_escape_that_closed_a_dialog_never_reaches_the_layer_the_focus_went_back_to() {
    let mut windows = Windows::new(ROOT);
    let mut list = drawn();
    list.windows[0]
        .layers
        .push(layer(DIALOG, Surface::Root(1), 0.0));
    windows.drew(list);
    windows.route(
        None,
        vec![button(150.0, true).into(), button(150.0, false).into()],
    );
    let had = windows.focused(None).map(|layer| layer.id);
    windows.focus(None, Some(DIALOG));

    // Escape's press reaches the dialog, which closes and gives the focus back to the screen.
    let pressed = windows.route(None, vec![escape(true)]);
    assert_eq!(
        pressed.iter().map(|told| told.target).collect::<Vec<_>>(),
        vec![DIALOG]
    );
    windows.focus(None, had);
    let released = windows.route(None, vec![escape(false)]);
    assert!(
        released.iter().all(|told| told.target != SCREEN),
        "the screen the focus went back to hears no release of a key it never heard pressed: \
         {released:?}"
    );
    assert_eq!(
        released.iter().map(|told| told.target).collect::<Vec<_>>(),
        vec![DIALOG],
        "the release goes where its press went"
    );
}

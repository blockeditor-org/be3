use super::*;

/// A dialog of the root's own, over the root's own surface.
const DIALOG: LayerId = LayerId(2);

#[test]
fn the_focus_moved_to_a_layer_and_back_tells_each_layer() {
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
    let told = |deliveries: Vec<Delivery>| -> Vec<(LayerId, Input)> {
        deliveries
            .into_iter()
            .map(|delivery| (delivery.target, delivery.input))
            .collect()
    };

    let had = windows.focused(None).map(|layer| layer.id);
    assert_eq!(had, Some(SCREEN), "the press focused the screen");
    assert_eq!(
        told(windows.focus(None, Some(DIALOG))),
        vec![(SCREEN, Input::Focus(false)), (DIALOG, Input::Focus(true))],
        "a dialog that opens takes the focus from the screen"
    );
    assert_eq!(windows.focused(None).map(|layer| layer.id), Some(DIALOG));
    assert_eq!(
        told(windows.focus(None, had)),
        vec![(DIALOG, Input::Focus(false)), (SCREEN, Input::Focus(true))],
        "closing, it gives the focus back to the screen that had it"
    );
    assert!(
        windows.focus(None, had).is_empty(),
        "focusing the layer that has the focus tells no one"
    );
}

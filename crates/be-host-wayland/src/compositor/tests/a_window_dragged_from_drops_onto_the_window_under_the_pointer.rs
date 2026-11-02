use super::*;
use crate::selection::client;

#[test]
fn a_window_dragged_from_drops_onto_the_window_under_the_pointer() {
    let mut harness = Harness::new();
    let _keyboard = harness.client.keyboard();
    let _pointer = harness.client.pointer();
    let device = harness.client.data_device();
    let (from, first) = harness.open();
    let (onto, second) = harness.open();
    let start = harness.window(first).center();
    let end = harness.window(second).center();
    assert!(!harness.window(first).contains(end));

    harness.frame(vec![
        Input::PointerMoved(start),
        Input::PointerButton {
            pos: start,
            button: PointerButton::Primary,
            pressed: true,
        },
    ]);
    harness.settle();
    let serial = harness
        .client
        .received
        .serial
        .expect("the press carried a serial");
    harness.client.start_drag(&device, &from.surface, serial);
    harness.settle();

    harness.frame(vec![Input::PointerMoved(pos2(end.x, end.y - 20.0))]);
    harness.frame(vec![Input::PointerMoved(end)]);
    harness.settle();
    assert_eq!(
        client::drop_target().as_ref(),
        Some(&onto.surface),
        "the drag enters the window under the pointer, not the one it began in"
    );

    harness.frame(vec![
        Input::PointerButton {
            pos: end,
            button: PointerButton::Primary,
            pressed: false,
        },
        Input::PointerMoved(start),
    ]);
    harness.settle();
    assert_eq!(
        client::dropped_on().as_ref(),
        Some(&onto.surface),
        "the drop lands where the button was let go, though the pointer moved on in the same frame"
    );
}

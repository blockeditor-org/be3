use super::*;

#[test]
fn a_button_beyond_the_five_reaches_the_window_with_its_own_code() {
    let mut harness = Harness::new();
    let _pointer = harness.client.pointer();
    let (_window, id) = harness.open();
    let at = harness.window(id).center();
    let button = |pressed| Input::PointerButton {
        pos: at,
        button: PointerButton::Other(0x115),
        pressed,
    };

    harness.frame(vec![Input::PointerMoved(at), button(true)]);
    harness.frame(vec![button(false)]);
    harness.settle();

    assert_eq!(
        harness.client.received.buttons,
        vec![(0x115, true), (0x115, false)],
        "the window hears BTN_FORWARD, not one of the five beui names"
    );
}

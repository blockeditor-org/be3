use super::*;

#[test]
fn a_press_between_two_popups_reaches_what_is_under_them() {
    let (mut harness, _device, _queue) = Harness::with_gpu();
    let _keyboard = harness.client.keyboard();
    let _pointer = harness.client.pointer();
    let (window, id) = harness.open();
    harness.click(&format!("wayland.window.{}", id.0));
    assert!(harness.client.received.activated);
    let shown = harness.window(id);
    let input = harness.rect("test.input");
    let above = (input.min.y - shown.min.y) as i32;

    let _below = harness.client.popup(
        harness.stepped.app_mut().compositor.server(),
        &window,
        (10, 280),
        (150, 200),
    );
    let _beside = harness.client.popup(
        harness.stepped.app_mut().compositor.server(),
        &window,
        (300, above),
        (50, 20),
    );
    harness.settle();
    let popups = harness.rect(&format!("wayland.popups.{}", id.0));
    let gap = pos2(shown.min.x + 100.0, input.center().y);
    assert!(popups.contains(gap), "the gap is inside the popups' bounds");
    assert!(input.contains(gap), "and over the text input");

    let presses = harness.client.received.buttons.len();
    harness.frame(vec![
        Input::PointerMoved(gap),
        Input::PointerButton {
            pos: gap,
            button: PointerButton::Primary,
            pressed: true,
        },
        Input::PointerButton {
            pos: gap,
            button: PointerButton::Primary,
            pressed: false,
        },
    ]);
    harness.settle();
    assert_eq!(
        harness.client.received.buttons.len(),
        presses,
        "the window hears nothing of a press between its popups"
    );
    assert!(
        !harness.client.received.activated,
        "the text input under the gap took the press and the focus"
    );
}

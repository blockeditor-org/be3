use crate::test_client::Seen;

use super::*;

#[test]
fn a_popup_reaching_past_its_window_keeps_its_size_and_is_shown_outside_it() {
    let (mut harness, _device, _queue) = Harness::with_gpu();
    let _pointer = harness.client.pointer();
    let (window, id) = harness.open();
    let shown = harness.window(id);

    let menu = harness.client.popup(
        harness.stepped.app_mut().compositor.server(),
        &window,
        (10, 280),
        (150, 200),
    );
    harness.settle();
    assert_eq!(
        harness.client.received.popup_geometry,
        Some((10, 280, 150, 200)),
        "the screen, not the window, is what the popup must fit in"
    );

    let popups = harness.rect(&format!("wayland.popups.{}", id.0));
    assert_eq!(
        popups,
        Rect::from_min_size(shown.min + Vec2::new(10.0, 280.0), Vec2::new(150.0, 200.0)),
        "the popup is shown where it is, past the window's edge"
    );

    let below = pos2(shown.min.x + 50.0, shown.max.y + 100.0);
    harness.frame(vec![
        Input::PointerMoved(below),
        Input::PointerButton {
            pos: below,
            button: PointerButton::Primary,
            pressed: true,
        },
    ]);
    harness.settle();
    assert!(
        harness
            .client
            .received
            .seen
            .contains(&Seen::Button(menu.surface.clone(), true)),
        "a press on the part outside the window reaches the popup"
    );
}

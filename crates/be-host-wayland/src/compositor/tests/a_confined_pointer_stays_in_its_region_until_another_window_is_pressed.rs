use super::*;
use crate::test_client::PointerSeen;
use beui::vec2;

#[test]
fn a_confined_pointer_stays_in_its_region_until_another_window_is_pressed() {
    let mut harness = Harness::new();
    let pointer = harness.client.pointer();
    let (window, id) = harness.open();
    let origin = harness.window(id).min;
    harness.click_at(origin + vec2(10.0, 10.0));
    let region = harness.client.region((0, 0, 50, 50));
    let _confined = harness.client.confine_pointer(&window.surface, &pointer, &region);
    window.surface.commit();
    harness.settle();
    assert_eq!(
        harness.client.received.protocols.pointer,
        [PointerSeen::Confined]
    );
    assert!(
        !harness
            .output
            .as_ref()
            .expect("a frame ran")
            .main()
            .window
            .requests
            .pointer_locked,
        "a confined pointer still moves on the host"
    );

    harness.frame(vec![Input::PointerMoved(origin + vec2(20.0, 30.0))]);
    harness.settle();
    assert_eq!(harness.client.received.pointer_at, Some((20.0, 30.0)));

    harness.frame(vec![Input::PointerMoved(origin + vec2(120.0, 30.0))]);
    harness.frame(vec![Input::PointerMoved(outside())]);
    harness.settle();
    assert_eq!(
        harness.client.received.pointer_at,
        Some((20.0, 30.0)),
        "the window never sees the pointer leave its region"
    );
    assert_eq!(
        harness.client.received.pointer_surface.as_ref(),
        Some(&window.surface),
        "nor leave its window"
    );

    harness.click("test.input");
    assert_eq!(
        harness.client.received.protocols.pointer,
        [PointerSeen::Confined, PointerSeen::Unconfined],
        "pressing elsewhere lets the pointer go"
    );
    assert_eq!(harness.compositor().keyboard_window(), None);
}

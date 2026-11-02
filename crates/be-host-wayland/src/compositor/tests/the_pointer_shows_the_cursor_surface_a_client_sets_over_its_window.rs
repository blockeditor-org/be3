use super::*;

#[test]
fn the_pointer_shows_the_cursor_surface_a_client_sets_over_its_window() {
    let (mut harness, _device, _queue) = Harness::with_gpu();
    let pointer = harness.client.pointer();
    let (_window, id) = harness.open();
    let center = harness.window(id).center();
    harness.frame(vec![Input::PointerMoved(center)]);
    harness.settle();
    assert!(
        harness.compositor().cursor_image().is_none(),
        "no cursor surface is shown before the client sets one"
    );

    let serial = harness
        .client
        .received
        .pointer_serial
        .expect("the pointer entered the window");
    let cursor = harness.client.surface();
    pointer.set_cursor(serial, Some(&cursor), 3, 4);
    harness.client.attach_surface(&cursor, 8, 8);
    harness.settle();
    let image = harness
        .compositor()
        .cursor_image()
        .expect("the client's cursor is shown");
    assert_eq!(image.size, Vec2::new(8.0, 8.0));
    assert_eq!(image.hotspot, Vec2::new(3.0, 4.0));

    harness.client.attach_surface(&cursor, 16, 16);
    harness.settle();
    let redrawn = harness
        .compositor()
        .cursor_image()
        .expect("the client's cursor is still shown");
    assert_eq!(redrawn.size, Vec2::new(16.0, 16.0));
    assert!(
        redrawn.revision > image.revision,
        "a commit to the cursor surface is a new revision to draw"
    );

    harness.frame(vec![Input::PointerMoved(outside())]);
    harness.settle();
    assert!(
        harness.compositor().cursor_image().is_none(),
        "away from the window the UI's own cursor is shown"
    );

    harness.frame(vec![Input::PointerMoved(center)]);
    harness.settle();
    let serial = harness
        .client
        .received
        .pointer_serial
        .expect("the pointer entered the window again");
    pointer.set_cursor(serial, None, 0, 0);
    harness.settle();
    assert!(harness.compositor().cursor_image().is_none());
    assert_eq!(
        harness
            .output
            .as_ref()
            .map(|output| output.main().window.requests.cursor),
        Some(CursorIcon::None),
        "a null cursor hides the pointer over the window"
    );
}

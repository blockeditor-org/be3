use super::*;

#[test]
fn a_cursor_surface_without_a_buffer_hides_the_pointer() {
    let mut harness = Harness::new();
    let pointer = harness.client.pointer();
    let (_window, id) = harness.open();
    let center = harness.window(id).center();
    harness.frame(vec![Input::PointerMoved(center)]);
    harness.settle();
    let shown = |harness: &Harness| {
        let window = &harness.output.as_ref().unwrap().main().window;
        (window.requests.cursor, window.cursor_image.clone())
    };
    let serial = harness
        .client
        .received
        .pointer_serial
        .expect("the pointer entered the window");
    let cursor = harness.client.surface();
    pointer.set_cursor(serial, Some(&cursor), 0, 0);
    harness.client.attach_surface(&cursor, 8, 8);
    harness.settle();
    assert_eq!(shown(&harness).0, CursorIcon::Default);
    assert!(shown(&harness).1.is_some(), "the client's cursor is shown");

    cursor.attach(None, 0, 0);
    cursor.commit();
    harness.settle();
    assert_eq!(
        shown(&harness),
        (CursorIcon::None, None),
        "an empty cursor surface, as GTK sets over a playing video, hides the pointer"
    );
    assert!(harness.compositor().cursor_image().is_none());
}

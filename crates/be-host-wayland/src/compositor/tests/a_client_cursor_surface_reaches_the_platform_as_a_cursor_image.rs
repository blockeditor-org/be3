use super::*;

#[test]
fn a_client_cursor_surface_reaches_the_platform_as_a_cursor_image() {
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
    assert_eq!(shown(&harness), (CursorIcon::Default, None));

    let serial = harness
        .client
        .received
        .pointer_serial
        .expect("the pointer entered the window");
    let cursor = harness.client.surface();
    pointer.set_cursor(serial, Some(&cursor), 3, 4);
    let half_red = [0x40, 0x00, 0x80, 0x80];
    harness
        .client
        .attach_surface_pixels(&cursor, 8, 8, &half_red.repeat(64));
    harness.settle();
    let (icon, image) = shown(&harness);
    let image = image.expect("the client's cursor is the window's cursor image");
    assert_eq!(icon, CursorIcon::Default, "where pictures are not shown");
    assert_eq!(image.size, Vec2::new(8.0, 8.0));
    assert_eq!(image.hotspot, Vec2::new(3.0, 4.0));
    assert_eq!(image.hotspot_pixels(), (3, 4));
    assert_eq!(
        &image.image.pixels()[..4],
        &[255, 0, 128, 128],
        "premultiplied ARGB becomes straight RGBA"
    );

    harness.settle();
    assert_eq!(
        shown(&harness).1.map(|still| still.image.id()),
        Some(image.image.id()),
        "frames without a commit keep the same picture"
    );

    harness.client.attach_surface(&cursor, 16, 16);
    harness.settle();
    let redrawn = shown(&harness)
        .1
        .expect("the client's cursor is still shown");
    assert_eq!(redrawn.size, Vec2::new(16.0, 16.0));
    assert_eq!(&redrawn.image.pixels()[..4], &[255, 255, 255, 255]);

    harness.frame(vec![Input::PointerMoved(outside())]);
    harness.settle();
    assert_eq!(
        shown(&harness).1,
        None,
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
    assert_eq!(
        shown(&harness),
        (CursorIcon::None, None),
        "a null cursor hides the pointer over the window"
    );
}

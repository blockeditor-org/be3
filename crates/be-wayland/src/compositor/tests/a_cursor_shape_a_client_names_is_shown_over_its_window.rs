use super::*;

use wayland_protocols::wp::cursor_shape::v1::client::{
    wp_cursor_shape_device_v1::Shape, wp_cursor_shape_manager_v1::WpCursorShapeManagerV1,
};

#[test]
fn a_cursor_shape_a_client_names_is_shown_over_its_window() {
    let mut harness = Harness::new();
    let pointer = harness.client.pointer();
    let shapes: WpCursorShapeManagerV1 = harness.client.bind("wp_cursor_shape_manager_v1", 1);
    let device = shapes.get_pointer(&pointer, &harness.client.handle, ());
    let (_window, id) = harness.open();
    let center = harness.window(id).center();
    harness.frame(vec![Input::PointerMoved(center)]);
    harness.settle();
    let serial = harness
        .client
        .received
        .pointer_serial
        .expect("the pointer entered the window");

    for (shape, icon) in [
        (Shape::Text, CursorIcon::Text),
        (Shape::ZoomIn, CursorIcon::ZoomIn),
        (Shape::NwResize, CursorIcon::ResizeNorthWest),
        (Shape::EwResize, CursorIcon::ResizeHorizontal),
        (Shape::Pointer, CursorIcon::PointingHand),
        (Shape::ContextMenu, CursorIcon::ContextMenu),
    ] {
        device.set_shape(serial, shape);
        harness.settle();
        assert_eq!(
            harness
                .output
                .as_ref()
                .map(|output| output.main().window.requests.cursor),
            Some(icon),
            "{shape:?} is shown as {icon:?}"
        );
        assert!(harness.compositor().cursor_image().is_none());
    }

    harness.frame(vec![Input::PointerMoved(outside())]);
    harness.settle();
    assert_eq!(
        harness
            .output
            .as_ref()
            .map(|output| output.main().window.requests.cursor),
        Some(CursorIcon::Default),
        "the shape is the window's alone"
    );
}

use super::*;

#[test]
fn the_view_a_screen_is_given_carries_its_scale_and_whether_it_was_moved() {
    let mut instances = placed();
    instances.next_screens(PASS);
    instances.set_view(
        INSTANCE,
        EditorView {
            rect: Rect::from_min_size(pos2(4.0, 6.0), vec2(200.0, 100.0)),
            scale: 0.5,
            moved: true,
        },
    );

    let opened = instances.next_screens(PASS).opened;

    assert!(opened.iter().any(|message| matches!(
        message,
        Message::Editor(EditorMessage::ViewChanged {
            instance,
            x,
            y,
            width,
            height,
            scale,
            moved,
        }) if *instance == INSTANCE
            && *x == 4.0
            && *y == 6.0
            && *width == 200.0
            && *height == 100.0
            && *scale == 0.5
            && *moved
    )));
}

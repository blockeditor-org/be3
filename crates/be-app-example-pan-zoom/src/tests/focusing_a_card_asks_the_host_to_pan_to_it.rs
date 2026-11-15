use super::*;

#[test]
fn focusing_a_card_asks_the_host_to_pan_to_it() {
    let (mut test, editor) = editor();

    test.set_view(
        Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(880.0, 580.0)),
        1.0,
    );
    test.run();
    let canvas = editor.content_rect();
    test.take_view_changes();

    test.click("pan_zoom.focus.6");

    let expected = canvas.center() - pos2(450.0, 490.0);
    let changes = test.take_view_changes();
    assert!(matches!(
        changes.as_slice(),
        [ViewChange::Pan(by)] if (by.x - expected.x).abs() < 0.01 && (by.y - expected.y).abs() < 0.01
    ));
}

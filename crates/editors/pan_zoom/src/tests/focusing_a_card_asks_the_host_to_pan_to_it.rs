use super::*;

#[test]
fn focusing_a_card_asks_the_host_to_pan_to_it() {
    let (mut editor, host) = editor();

    host.set_beui_view(
        Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(880.0, 580.0)),
        1.0,
    );
    editor.run();
    let canvas = canvas(&mut editor);

    editor.click("pan_zoom.focus.6");
    editor.run();

    let expected = canvas.center() - pos2(450.0, 490.0);
    let changes = host.take_view_changes();
    assert!(matches!(
        changes.as_slice(),
        [ViewChange::Pan { x, y }] if (*x - expected.x).abs() < 0.01 && (*y - expected.y).abs() < 0.01
    ));
}

use super::*;

#[test]
fn zooming_asks_the_host_instead_of_moving_the_view() {
    let (mut test, _editor) = editor();

    test.click("pan_zoom.zoom_in");
    test.run();

    let changes = test.take_view_changes();
    assert!(matches!(
        changes.as_slice(),
        [ViewChange::Zoom { factor, .. }] if *factor > 1.0
    ));
}

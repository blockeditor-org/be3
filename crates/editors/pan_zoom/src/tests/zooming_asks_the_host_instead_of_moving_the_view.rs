use super::*;

#[test]
fn zooming_asks_the_host_instead_of_moving_the_view() {
    let (mut editor, host) = editor();

    editor.click("pan_zoom.zoom_in");
    editor.run();

    let changes = host.take_view_changes();
    assert!(matches!(
        changes.as_slice(),
        [ViewChange::Zoom { factor, .. }] if *factor > 1.0
    ));
}

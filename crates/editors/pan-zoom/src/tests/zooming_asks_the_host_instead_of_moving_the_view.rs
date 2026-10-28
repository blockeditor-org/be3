use super::*;

#[test]
fn zooming_asks_the_host_instead_of_moving_the_view() {
    let (mut test, _editor) = editor();
    test.run();
    assert!(
        matches!(
            test.take_view_changes().as_slice(),
            [ViewChange::KeepFitted(Some(_))]
        ),
        "the editor says once what the host keeps fitted"
    );

    test.click("pan_zoom.zoom_in");

    let changes = test.take_view_changes();
    assert!(matches!(
        changes.as_slice(),
        [ViewChange::Zoom { factor, .. }] if *factor > 1.0
    ));
}

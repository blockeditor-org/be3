use super::*;

/// The example declares `pan_and_zoom` and has no zoom code of its own: the frame registers
/// `editor.zoom-in` and the rest, the palette runs them, and its zoom bar shows them.
#[test]
fn the_frame_zooms_an_editor_with_no_zoom_code_of_its_own_from_the_palette() {
    let (mut test, _editor) = editor();
    test.run();
    test.take_view_changes();

    test.run_action("editor.zoom-in");
    assert!(matches!(
        test.take_view_changes().as_slice(),
        [ViewChange::Zoom { factor, .. }] if (*factor - be_sdk_beui::beui::unstyled::ZOOM_STEP).abs() < 1e-6
    ));
    test.run_action("editor.zoom-out");
    assert!(matches!(
        test.take_view_changes().as_slice(),
        [ViewChange::Zoom { factor, .. }] if *factor < 1.0
    ));
    assert!(
        test.shown("editor.zoom_bar"),
        "the zoom bar shows the frame's actions"
    );
    assert!(
        test.shown("editor.zoom_fit"),
        "with fit, as it keeps the cards fitted"
    );
}

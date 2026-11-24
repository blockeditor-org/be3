use be_sdk_api::Vec2 as ViewVec2;

use super::*;

#[test]
fn a_child_placed_to_fit_its_content_is_asked_for_at_most_that_room() {
    let mut placement = placed(rect((0.0, 0.0), (300.0, 400.0)), ChildMode::Passive);
    placement.fit = Some(ViewVec2 { x: 300.0, y: 250.0 });
    let screen = child_screen(&placement, EditorInstanceId(5), 2.0, 1.0);
    assert!(
        screen.fit,
        "it takes the smaller of that room and its content"
    );
    assert_eq!(
        (screen.metrics.logical.x, screen.metrics.logical.y),
        (300.0, 250.0)
    );

    // A preview of it is drawn the size of its place, never fitted.
    let preview = preview_screen(&placement, EditorInstanceId(5), 2.0);
    assert!(!preview.fit);
    assert_eq!(
        (preview.metrics.logical.x, preview.metrics.logical.y),
        (300.0, 400.0)
    );
}

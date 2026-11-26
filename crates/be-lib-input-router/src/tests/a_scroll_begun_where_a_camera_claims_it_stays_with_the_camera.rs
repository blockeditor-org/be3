use super::*;

/// A gesture claim: a held scroll begun where the canvas's camera takes gestures over an editor
/// goes to the canvas, and stays with it after the editor slides under the pointer.
#[test]
fn a_scroll_begun_where_a_camera_claims_it_stays_with_the_camera() {
    let mut test = canvas_with_editor();
    test.surfaces[2].gestures = Some(rect(200.0, 200.0, 200.0, 20.0));
    let mut router = Router::default();
    assert_eq!(
        router.gesture(&test, pos2(250.0, 210.0), Phase::Began),
        Some(CANVAS),
        "the editor grants the canvas the gesture along its top edge"
    );
    assert_eq!(
        router.gesture(&test, pos2(250.0, 250.0), Phase::Moved),
        Some(CANVAS)
    );
    assert_eq!(
        router.gesture(&test, pos2(250.0, 250.0), Phase::Ended),
        Some(CANVAS)
    );
    assert_eq!(
        router.gesture(&test, pos2(250.0, 250.0), Phase::Moved),
        Some(EDITOR),
        "a wheel with no gesture held goes where the pointer is"
    );
}

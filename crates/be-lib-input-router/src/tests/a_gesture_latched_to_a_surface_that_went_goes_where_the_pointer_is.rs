use super::*;

/// A surface removed mid-gesture takes the latch with it, so the wheel is not eaten.
#[test]
fn a_gesture_latched_to_a_surface_that_went_goes_where_the_pointer_is() {
    let test = canvas_with_editor();
    let mut router = Router::default();
    assert_eq!(
        router.gesture(&test, pos2(250.0, 250.0), Phase::Began),
        Some(EDITOR)
    );
    router.forget(EDITOR);
    assert_eq!(router.latched(), None);
    let gone = Rects::default()
        .with(ROOT, rect(0.0, 0.0, 1000.0, 1000.0), None)
        .with(CANVAS, rect(100.0, 100.0, 600.0, 600.0), Some(ROOT));
    assert_eq!(
        router.gesture(&gone, pos2(250.0, 250.0), Phase::Moved),
        Some(CANVAS)
    );
}

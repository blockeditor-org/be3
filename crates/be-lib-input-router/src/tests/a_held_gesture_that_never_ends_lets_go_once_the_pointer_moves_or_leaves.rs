use super::*;

fn wheel() -> Input {
    Input::Scroll(be_lib_ui_types::input::ScrollDelta::Pixels(
        be_lib_geometry::vec2(0.0, 4.0),
    ))
}

/// A touchpad scroll whose end is lost (an axis stop the platform dropped) must not hold every
/// later wheel: the latch lets go once the pointer moves, leaves or a press is cancelled, or the
/// window loses the keyboard.
#[test]
fn a_held_gesture_that_never_ends_lets_go_once_the_pointer_moves_or_leaves() {
    let test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    let begin_over_editor = |router: &mut Router<u32>| {
        router.route(&test, Input::PointerMoved(pos2(250.0, 250.0)));
        assert_eq!(
            targets(&router.route(&test, Input::ScrollStarted)),
            vec![EDITOR]
        );
        assert_eq!(router.latched(), Some(Some(EDITOR)));
    };

    begin_over_editor(&mut router);
    router.route(&test, Input::PointerRested(pos2(250.0, 250.0)));
    assert_eq!(
        router.latched(),
        Some(Some(EDITOR)),
        "a pointer resting where it was keeps it"
    );
    router.route(&test, Input::PointerMoved(pos2(251.5, 250.0)));
    assert_eq!(
        router.latched(),
        Some(Some(EDITOR)),
        "so does a move within the slop, as a finger landing on the touchpad jitters it"
    );
    router.route(&test, Input::PointerMoved(pos2(150.0, 150.0)));
    assert_eq!(
        targets(&router.route(&test, wheel())),
        vec![CANVAS],
        "a plain wheel after the pointer moved goes where it is now"
    );

    begin_over_editor(&mut router);
    router.route(&test, Input::PointerLeft);
    assert_eq!(router.latched(), None, "the pointer left the window");

    begin_over_editor(&mut router);
    router.route(&test, Input::Focus(false));
    assert_eq!(router.latched(), None, "the window lost the keyboard");

    begin_over_editor(&mut router);
    router.route(&test, Input::PointersCancelled);
    assert_eq!(router.latched(), None, "the pointers were cancelled");
}

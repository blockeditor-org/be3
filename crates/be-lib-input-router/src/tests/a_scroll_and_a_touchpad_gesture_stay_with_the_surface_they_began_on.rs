use super::*;

#[test]
fn a_scroll_and_a_touchpad_gesture_stay_with_the_surface_they_began_on() {
    let test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    router.route(&test, Input::PointerMoved(pos2(250.0, 250.0)));
    assert_eq!(
        targets(&router.route(&test, Input::ScrollStarted)),
        vec![EDITOR]
    );
    router.route(&test, Input::PointerMoved(pos2(150.0, 150.0)));
    assert_eq!(
        targets(&router.route(
            &test,
            Input::Scroll(be_lib_ui_types::input::ScrollDelta::Pixels(
                be_lib_geometry::vec2(0.0, 4.0)
            ))
        )),
        vec![EDITOR],
        "a scroll that began over the editor stays with it"
    );
    assert_eq!(
        targets(&router.route(&test, Input::ScrollEnded)),
        vec![EDITOR]
    );
    assert_eq!(
        targets(&router.route(&test, Input::Touchpad(TouchpadGesture::PinchBegan(2)))),
        vec![CANVAS],
        "the next gesture begins where the pointer is now"
    );
    router.route(&test, Input::PointerMoved(pos2(250.0, 250.0)));
    assert_eq!(
        targets(&router.route(&test, Input::Touchpad(TouchpadGesture::PinchEnded(false)))),
        vec![CANVAS]
    );
    assert_eq!(
        targets(&router.route(&test, Input::Touchpad(TouchpadGesture::HoldBegan(2)))),
        vec![EDITOR]
    );
}

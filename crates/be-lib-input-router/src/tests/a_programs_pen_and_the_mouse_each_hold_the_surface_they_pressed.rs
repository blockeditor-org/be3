use super::*;

/// A Wayland program's window, or any catcher: each pointer holds what it pressed, apart from
/// the others, and a finger holds only itself.
#[test]
fn a_programs_pen_and_the_mouse_each_hold_the_surface_they_pressed() {
    let test = canvas_with_editor();
    let mut router = Router::default();
    let at = |x, y| pos2(x, y);
    assert_eq!(
        router.press(&test, Pointer::Pen, PointerButton::Primary, at(250.0, 250.0)),
        Some(EDITOR)
    );
    assert_eq!(
        router.press(&test, Pointer::Mouse, PointerButton::Primary, at(150.0, 150.0)),
        Some(CANVAS),
        "the mouse is not held by the pen's press"
    );
    assert_eq!(router.moved(&test, Pointer::Pen, at(150.0, 150.0)), Some(EDITOR));
    assert_eq!(router.moved(&test, Pointer::Mouse, at(250.0, 250.0)), Some(CANVAS));
    let finger = |id| Pointer::Touch(TouchId { device: 0, finger: id });
    assert_eq!(
        router.press(&test, finger(1), PointerButton::Primary, at(250.0, 250.0)),
        Some(EDITOR)
    );
    assert_eq!(
        router.press(&test, finger(2), PointerButton::Primary, at(150.0, 150.0)),
        Some(CANVAS),
        "a second finger goes where it lands"
    );
    assert_eq!(router.cancel(Pointer::Pen), Some(EDITOR));
    assert_eq!(router.release(finger(2), PointerButton::Primary), Some(CANVAS));
    assert_eq!(router.held(finger(1)), Some(EDITOR));
}

use super::*;

#[test]
fn a_press_and_release_happen_where_the_pointer_is() {
    for (button, down, up, data) in [
        (PointerButton::Primary, MOUSE_LEFT_DOWN, MOUSE_LEFT_UP, 0),
        (
            PointerButton::Secondary,
            MOUSE_RIGHT_DOWN,
            MOUSE_RIGHT_UP,
            0,
        ),
        (PointerButton::Middle, MOUSE_MIDDLE_DOWN, MOUSE_MIDDLE_UP, 0),
        (PointerButton::Back, MOUSE_X_DOWN, MOUSE_X_UP, XBUTTON1),
        (PointerButton::Forward, MOUSE_X_DOWN, MOUSE_X_UP, XBUTTON2),
    ] {
        let at = PixelPoint::new(10, 20);
        let moved = MOUSE_MOVE | MOUSE_ABSOLUTE | MOUSE_VIRTUAL_DESK;
        let to = absolute(DESKTOP, at);
        assert_eq!(
            pointer(DESKTOP, at, PointerAction::Press(button)),
            [Synth::Mouse {
                x: to.x,
                y: to.y,
                data,
                flags: moved | down
            }],
            "{button:?}"
        );
        assert_eq!(
            pointer(DESKTOP, at, PointerAction::Release(button)),
            [Synth::Mouse {
                x: to.x,
                y: to.y,
                data,
                flags: moved | up
            }],
            "{button:?}"
        );
    }
    assert_eq!(button_flags(PointerButton::Other(9), true), None);
    assert_eq!(
        pointer(
            DESKTOP,
            PixelPoint::new(0, 0),
            PointerAction::Press(PointerButton::Other(9))
        )
        .len(),
        1
    );
}

use super::*;

#[test]
fn touch_emulation_turns_the_mouse_into_a_finger() {
    let mut intaken = Intaken::new();
    intaken.runner.context().set_touch_emulation(true);
    let finger = |phase, pos| Event::Touch {
        id: TouchId {
            device: 0,
            finger: 0,
        },
        phase,
        pos,
        force: None,
    };

    intaken.runner.push(Input::PointerButton {
        pos: pos2(20.0, 40.0),
        button: PointerButton::Primary,
        pressed: true,
    });
    intaken.runner.push(Input::PointerMoved(pos2(30.0, 40.0)));
    intaken.runner.push(Input::PointerButton {
        pos: pos2(30.0, 40.0),
        button: PointerButton::Secondary,
        pressed: true,
    });
    intaken.runner.push(Input::PointerButton {
        pos: pos2(30.0, 40.0),
        button: PointerButton::Primary,
        pressed: false,
    });
    let seen = intaken.frame(2.0, []);

    assert_eq!(
        seen,
        [
            finger(TouchPhase::Start, pos2(10.0, 20.0)),
            finger(TouchPhase::Move, pos2(15.0, 20.0)),
            finger(TouchPhase::End, pos2(15.0, 20.0)),
        ],
        "only the primary button is a finger, and positions arrive in points"
    );
    assert_eq!(
        intaken.runner.window().requests.cursor,
        CursorIcon::Touch,
        "every platform is asked to show the finger as the touch cursor"
    );

    intaken.runner.push(Input::PointerButton {
        pos: pos2(20.0, 40.0),
        button: PointerButton::Primary,
        pressed: true,
    });
    intaken.frame(2.0, []);
    intaken.runner.context().set_touch_emulation(false);
    assert_eq!(
        intaken.frame(2.0, []),
        [finger(TouchPhase::Cancel, pos2(10.0, 20.0))],
        "turning emulation off mid-touch cancels the finger at the next frame"
    );
    assert_eq!(
        intaken.frame(2.0, [Input::PointerMoved(pos2(22.0, 40.0)).into()]),
        [Event::PointerMoved(pos2(11.0, 20.0))],
        "after which the mouse is a mouse again"
    );
    assert_eq!(intaken.runner.window().requests.cursor, CursorIcon::Default);
}

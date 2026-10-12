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

    intaken
        .runner
        .pointer_button(pos2(20.0, 40.0), PointerButton::Primary, true);
    intaken.runner.push(Event::PointerMoved(pos2(30.0, 40.0)));
    intaken
        .runner
        .pointer_button(pos2(30.0, 40.0), PointerButton::Secondary, true);
    intaken
        .runner
        .pointer_button(pos2(30.0, 40.0), PointerButton::Primary, false);
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

    intaken
        .runner
        .pointer_button(pos2(20.0, 40.0), PointerButton::Primary, true);
    intaken.frame(2.0, []);
    intaken.runner.context().set_touch_emulation(false);
    let seen = intaken.frame(2.0, [Event::PointerMoved(pos2(22.0, 40.0)).into()]);
    assert_eq!(
        seen,
        [
            finger(TouchPhase::Cancel, pos2(10.0, 20.0)),
            Event::PointerMoved(pos2(11.0, 20.0)),
        ],
        "turning emulation off mid-touch cancels the finger"
    );
}

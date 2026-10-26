use super::*;

#[test]
fn modifiers_are_replayed_only_when_they_change() {
    let mut replay = Replay::default();
    let shifted = |phase| {
        let mut event = pointer(PointerId::Mouse, phase, pos2(100.0, 50.0));
        event.modifiers = Modifiers::SHIFT;
        sent(event)
    };
    assert_eq!(
        replayed(
            &mut replay,
            vec![
                shifted(PointerPhase::Down(PRIMARY)),
                shifted(PointerPhase::Up(PRIMARY)),
                Forwarded::Input(Event::Modifiers(Modifiers::SHIFT)),
                key(Key::A, Modifiers::SHIFT, None),
            ],
            1.0
        ),
        [
            Input::Modifiers(Modifiers::SHIFT),
            button(Pos2::ZERO, true),
            button(Pos2::ZERO, false),
            Input::Key(Keystroke {
                key: Some(Key::A),
                pressed: true,
                repeat: false,
                evdev: Some(30),
                text: None,
            }),
        ]
    );
    assert_eq!(
        replayed(
            &mut replay,
            vec![Forwarded::Input(Event::Modifiers(Modifiers::NONE))],
            1.0
        ),
        [Input::Modifiers(Modifiers::NONE)]
    );
    assert_eq!(
        replay.intercepted(None, Modifiers::CTRL),
        [Input::Modifiers(Modifiers::CTRL)],
        "an intercepted key brings the modifiers held with it"
    );
}

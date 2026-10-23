use super::*;

#[test]
fn losing_focus_lets_go_of_the_modifiers_but_not_an_emulated_touch() {
    let mut intaken = Intaken::new();
    intaken.runner.context().set_touch_emulation(true);
    intaken.frame(1.0, [Input::Modifiers(Modifiers::SHIFT).into()]);
    intaken.runner.push(Input::PointerButton {
        pos: pos2(4.0, 6.0),
        button: PointerButton::Primary,
        pressed: true,
    });
    intaken.frame(1.0, []);

    let seen = intaken.frame(1.0, [Input::Focus(false).into()]);
    assert_eq!(
        seen,
        [Event::Modifiers(Modifiers::NONE), Event::Focus(false)],
        "the keyboard going elsewhere leaves the press alone"
    );

    let seen = intaken.frame(1.0, [Input::PointersCancelled.into()]);
    assert_eq!(
        seen,
        [
            Event::Touch {
                id: TouchId {
                    device: 0,
                    finger: 0,
                },
                phase: TouchPhase::Cancel,
                pos: pos2(4.0, 6.0),
                force: None,
            },
            Event::PointersCancelled,
        ]
    );
}

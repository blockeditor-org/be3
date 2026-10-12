use super::*;

#[test]
fn losing_focus_lets_go_of_the_modifiers_and_an_emulated_touch() {
    let mut intaken = Intaken::new();
    intaken.runner.context().set_touch_emulation(true);
    intaken.frame(1.0, [Event::Modifiers(Modifiers::SHIFT).into()]);
    intaken
        .runner
        .pointer_button(pos2(4.0, 6.0), PointerButton::Primary, true);
    intaken.frame(1.0, []);

    let seen = intaken.frame(1.0, [Event::Focus(false).into()]);
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
            Event::Modifiers(Modifiers::NONE),
            Event::Focus(false),
        ]
    );
}

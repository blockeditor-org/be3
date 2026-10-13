use super::*;
use crate::styled::TextInput;

#[test]
fn tapping_the_focused_text_input_again_asks_for_the_keyboard_again() {
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <TextInput value="" />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let at = harness.center(input);

    harness.touch(TouchPhase::Start, at);
    let tapped = harness.frame(vec![touch_event(1, TouchPhase::End, at)]);
    assert!(
        tapped.show_keyboard,
        "a tap that focuses a text input asks for the keyboard"
    );
    let area = tapped.window.ime;
    assert!(area.as_ref().is_some_and(|area| area.keyboard), "{area:?}");

    let settled = harness.frame(Vec::new());
    assert!(
        !settled.show_keyboard,
        "the keyboard is asked for once a tap"
    );
    assert_eq!(settled.window.ime, area, "nothing about the field changed");

    harness.touch(TouchPhase::Start, at);
    let again = harness.frame(vec![touch_event(1, TouchPhase::End, at)]);
    assert_eq!(
        again.window.ime, area,
        "the field asks for the same input method"
    );
    assert!(
        again.show_keyboard,
        "tapping the field again asks for the keyboard again, which the user may have closed"
    );

    harness.touch(TouchPhase::Start, at);
    harness.touch(TouchPhase::Move, at + vec2(0.0, 40.0));
    let dragged = harness.frame(vec![touch_event(1, TouchPhase::End, at + vec2(0.0, 40.0))]);
    assert!(
        !dragged.show_keyboard,
        "a drag that starts on the field is no tap"
    );
}

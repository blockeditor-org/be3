use super::*;
use crate::native::keyboard::styled::key_center;
use crate::native::keyboard::unstyled::{Layer, Value};
use crate::reactive::*;
use crate::styled::*;

#[test]
fn the_simulated_mouse_opens_the_on_screen_keyboard_and_fingers_type_on_it() {
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <TextInput value=String::new() />
        }]
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.context().enable_keyboard();
    harness.frame(Vec::new());
    harness.enable_mouse_simulation();
    harness.key(Key::Tab, Modifiers::NONE);

    let toggle = harness.simulated_button(3);
    harness.finger(1, TouchPhase::Start, toggle);
    harness.finger(1, TouchPhase::End, toggle);
    harness.frame(Vec::new());
    let keyboard = harness
        .context()
        .keyboard_rect()
        .expect("the simulation's keyboard button opens the on-screen keyboard");
    assert!(
        keyboard.bottom() <= toggle.y,
        "the keyboard sits above the simulation's strip"
    );

    for letter in ['h', 'i'] {
        let key = key_center(keyboard, Layer::Text, Value::Char(letter)).expect("a letter key");
        harness.finger(2, TouchPhase::Start, key);
        harness.finger(2, TouchPhase::End, key);
    }
    harness.frame(Vec::new());
    assert_eq!(
        harness
            .document()
            .api::<styled::TextInputApi>(input)
            .value(),
        "hi",
        "fingers on the keyboard press its keys rather than moving the simulated mouse"
    );

    harness.finger(1, TouchPhase::Start, toggle);
    harness.finger(1, TouchPhase::End, toggle);
    harness.frame(Vec::new());
    assert!(
        harness.context().keyboard_rect().is_none(),
        "the button closes it again"
    );
}

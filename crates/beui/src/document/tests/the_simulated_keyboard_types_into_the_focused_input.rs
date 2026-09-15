use super::*;
use crate::reactive::view;
use crate::styled::TextInput;

#[test]
fn the_simulated_keyboard_types_into_the_focused_input() {
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <TextInput value=String::new() />
        }]
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.enable_mouse_simulation();
    harness.key(Key::Tab, Modifiers::NONE);

    let keyboard = harness.simulated_button(3);
    harness.finger(1, TouchPhase::Start, keyboard);
    harness.finger(1, TouchPhase::End, keyboard);

    for label in ["h", "i"] {
        let key = harness.simulated_key(label);
        harness.finger(2, TouchPhase::Start, key);
        harness.finger(2, TouchPhase::End, key);
    }
    harness.frame(Vec::new());

    assert_eq!(styled::text_input_value(harness.document(), input), "hi");
}

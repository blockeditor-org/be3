use super::*;

use beui::Modifiers;
use beui::input::typing::typed;

use crate::state::KeyboardConfig;

const KEY_Y: u32 = 21;
const KEY_Z: u32 = 44;

fn type_in(harness: &mut Harness, text: &str) -> Vec<(u32, u32)> {
    let mut events = Vec::new();
    typed(text, Modifiers::NONE, &mut events);
    harness.client.received.leveled.clear();
    harness.frame(events);
    harness.settle();
    std::mem::take(&mut harness.client.received.leveled)
}

#[test]
fn switching_the_layout_reaches_programs_and_keys_typed_without_a_place() {
    let mut harness = Harness::new();
    let _keyboard = harness.client.keyboard();
    let _pointer = harness.client.pointer();
    let (_window, id) = harness.open();
    let text_input = harness.client.text_input();
    harness.click(&format!("wayland.window.{}", id.0));
    text_input.enable();
    text_input.commit();
    harness.settle();
    let config = KeyboardConfig {
        layout: "us,de".to_owned(),
        variant: String::new(),
        options: String::new(),
        repeat_delay: 600,
        repeat_rate: 25,
    };
    assert!(harness.compositor().set_keyboard(&config));
    harness.settle();
    assert_eq!(harness.client.received.group, 0);
    assert_eq!(
        type_in(&mut harness, "y"),
        [(KEY_Y, 0)],
        "the first layout is active to begin with"
    );

    harness.compositor().set_layout(1);
    harness.settle();
    assert_eq!(
        harness.client.received.group, 1,
        "the focused program is told the new layout through the keyboard's group"
    );
    assert_eq!(
        type_in(&mut harness, "y"),
        [(KEY_Z, 0)],
        "the on-screen keyboard's keys are found in the new layout, where y is on Z's key"
    );

    harness.compositor().set_layout(2);
    harness.settle();
    assert_eq!(
        harness.client.received.group, 1,
        "a layout past the last is no switch"
    );

    harness.compositor().set_layout(0);
    harness.settle();
    assert_eq!(harness.client.received.group, 0);
    assert_eq!(type_in(&mut harness, "y"), [(KEY_Y, 0)]);
}

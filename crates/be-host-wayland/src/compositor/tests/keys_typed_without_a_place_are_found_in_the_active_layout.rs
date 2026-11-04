use super::*;

use beui::Modifiers;
use beui::input::typing::{press_chord, typed};

use crate::state::KeyboardConfig;

const SHIFT: u32 = 1;
const CTRL: u32 = 4;
const LEVEL3: u32 = 128;
const KEY_Q: u32 = 16;
const KEY_Y: u32 = 21;
const KEY_C: u32 = 46;
const KEY_A: u32 = 30;
const KEY_Z: u32 = 44;
const KEY_LEFTSHIFT: u32 = 42;
const KEY_LEFTCTRL: u32 = 29;

fn layout(harness: &mut Harness, layout: &str) {
    let config = KeyboardConfig {
        layout: layout.to_owned(),
        variant: String::new(),
        options: String::new(),
        repeat_delay: 600,
        repeat_rate: 25,
    };
    assert!(harness.compositor().set_keyboard(&config));
}

fn type_in(harness: &mut Harness, text: &str) -> Vec<(u32, u32)> {
    let mut events = Vec::new();
    typed(text, Modifiers::NONE, &mut events);
    harness.client.received.leveled.clear();
    harness.frame(events);
    harness.settle();
    std::mem::take(&mut harness.client.received.leveled)
}

#[test]
fn keys_typed_without_a_place_are_found_in_the_active_layout() {
    let mut harness = Harness::new();
    let _keyboard = harness.client.keyboard();
    let _pointer = harness.client.pointer();
    let (_window, id) = harness.open();
    let text_input = harness.client.text_input();
    harness.click(&format!("wayland.window.{}", id.0));
    text_input.enable();
    text_input.commit();
    harness.settle();

    layout(&mut harness, "de");
    assert_eq!(
        type_in(&mut harness, "yz"),
        [(KEY_Z, 0), (KEY_Y, 0)],
        "on a German layout y and z are each other's keys, so each letter is sent from where it \
         is"
    );
    assert_eq!(
        type_in(&mut harness, "@A"),
        [
            (KEY_LEFTSHIFT, 0),
            (KEY_Q, LEVEL3),
            (KEY_LEFTSHIFT, 0),
            (KEY_A, SHIFT)
        ],
        "a character on another level is sent with that level's modifiers and no others: @, \
         typed with Shift as on a US layout, is AltGr+Q there, and a capital is Shift with its \
         letter"
    );
    assert_eq!(
        harness.client.received.depressed, 0,
        "the modifiers go back to what is held once the key is sent"
    );

    let mut chord = Vec::new();
    press_chord(Key::Z, Modifiers::CTRL, Modifiers::NONE, &mut chord);
    harness.client.received.leveled.clear();
    harness.frame(chord);
    harness.settle();
    assert_eq!(
        harness.client.received.leveled,
        [(KEY_LEFTCTRL, 0), (KEY_Y, CTRL)],
        "a shortcut is sent from the key its letter is on"
    );

    layout(&mut harness, "ru");
    harness.client.received.protocols.text_committed.clear();
    assert_eq!(
        type_in(&mut harness, "a"),
        [],
        "a letter the layout has no key for is no key"
    );
    assert_eq!(
        harness.client.received.protocols.text_committed,
        ["a"],
        "it reaches the field as committed text instead"
    );
    let mut chord = Vec::new();
    press_chord(Key::C, Modifiers::CTRL, Modifiers::NONE, &mut chord);
    harness.client.received.leveled.clear();
    harness.frame(chord);
    harness.settle();
    assert_eq!(
        harness.client.received.leveled,
        [(KEY_LEFTCTRL, 0), (KEY_C, CTRL)],
        "a shortcut whose letter is on no key is sent from its US place, as Latin shortcuts are \
         matched on other scripts' layouts"
    );
}

use std::time::{Duration, Instant};

use beui::{Event, Forwarded, ForwardedInput, KeyPress, NodeId};

use super::super::held_keys;
use be_sdk_api::EditorInstanceId;

const DECK: &str = "be3.presentation";
const PLACER: EditorInstanceId = EditorInstanceId(6);
const OWN: NodeId = NodeId::new(90, 1);
const NOTES: NodeId = NodeId::new(120, 1);

fn input(events: Vec<Forwarded>) -> ForwardedInput {
    ForwardedInput {
        events,
        rect: beui::Rect::ZERO,
        hovered: false,
        focused: true,
        pointer: None,
        modifiers: beui::Modifiers::NONE,
    }
}

fn letter(text: &str) -> Forwarded {
    Forwarded::Input(Event::Key {
        press: KeyPress {
            key: beui::Key::A,
            pressed: true,
            repeat: false,
            modifiers: beui::Modifiers::NONE,
        },
        evdev: None,
        text: Some(text.to_owned()),
    })
}

#[test]
fn keys_typed_before_a_child_takes_the_keyboard_wait_for_it() {
    let start = Instant::now();
    let at = |ms: u64| start + Duration::from_millis(ms);

    let kept = held_keys::route(
        DECK,
        PLACER,
        Some(OWN),
        true,
        at(0),
        input(vec![letter("T")]),
    );
    assert!(
        kept.events.is_empty(),
        "the deck does not hear a key meant for its notes"
    );
    let kept = held_keys::route(
        DECK,
        PLACER,
        Some(OWN),
        true,
        at(10),
        input(vec![letter("h")]),
    );
    assert!(kept.events.is_empty());

    held_keys::given(DECK, PLACER, NOTES, at(20));
    let early = held_keys::route(
        "be3.text",
        EditorInstanceId(10),
        Some(NOTES),
        false,
        at(25),
        input(vec![Forwarded::Focus(true)]),
    );
    assert_eq!(
        early.events,
        [Forwarded::Focus(true)],
        "the keys wait until the notes can take text"
    );
    assert!(!held_keys::owed_now(NOTES, false, at(30)));
    assert!(
        held_keys::owed_now(NOTES, true, at(40)),
        "the notes asked for an input method"
    );
    let given = held_keys::route(
        "be3.text",
        EditorInstanceId(10),
        Some(NOTES),
        false,
        at(40),
        input(Vec::new()),
    );
    assert_eq!(
        given.events,
        [letter("T"), letter("h")],
        "in the order they were typed"
    );

    let after = held_keys::route(
        DECK,
        PLACER,
        Some(OWN),
        false,
        at(50),
        input(vec![letter("x")]),
    );
    assert_eq!(
        after.events,
        [letter("x")],
        "with no gift pending the deck hears its keys"
    );

    held_keys::route(
        DECK,
        PLACER,
        Some(OWN),
        true,
        at(60),
        input(vec![letter("y")]),
    );
    held_keys::refused(DECK, PLACER, at(70));
    let back = held_keys::route(DECK, PLACER, Some(OWN), false, at(80), input(Vec::new()));
    assert_eq!(
        back.events,
        [letter("y")],
        "a gift that falls through gives the deck its keys back"
    );
}

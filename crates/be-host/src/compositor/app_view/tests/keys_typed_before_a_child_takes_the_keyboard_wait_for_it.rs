use beui::{Event, Forwarded, ForwardedInput, KeyPress, Modifiers, NodeId};

use super::super::held_keys::{self, Routed};
use be_sdk_api::EditorInstanceId;

const DECK: &str = "be3.presentation";
const TEXT: &str = "be3.text";
const PLACER: EditorInstanceId = EditorInstanceId(6);
const NOTES_EDITOR: EditorInstanceId = EditorInstanceId(10);
const DECK_VIEW: NodeId = NodeId::new(90, 1);
const NOTES_VIEW: NodeId = NodeId::new(120, 1);

fn input(events: Vec<Forwarded>) -> ForwardedInput {
    ForwardedInput {
        events,
        rect: beui::Rect::ZERO,
        hovered: false,
        focused: true,
        pointer: None,
        modifiers: Modifiers::NONE,
    }
}

fn letter(text: &str) -> Forwarded {
    Forwarded::Input(Event::Key {
        press: KeyPress {
            key: beui::Key::A,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        },
        evdev: None,
        text: Some(text.to_owned()),
    })
}

fn deck(pending: bool, events: Vec<Forwarded>) -> (Vec<Forwarded>, Routed) {
    let mut input = input(events);
    let routed = held_keys::route(DECK, PLACER, Some(DECK_VIEW), pending, (4, 4), &mut input);
    (input.events, routed)
}

fn notes(frames: (u64, u64), events: Vec<Forwarded>) -> Vec<Forwarded> {
    let mut input = input(events);
    held_keys::route(TEXT, NOTES_EDITOR, Some(NOTES_VIEW), false, frames, &mut input);
    input.events
}

#[test]
fn keys_typed_before_a_child_takes_the_keyboard_wait_for_it() {
    assert_eq!(deck(true, vec![letter("T")]).0, [], "the deck does not hear a key meant for its notes");
    assert_eq!(deck(true, vec![letter("h")]).0, []);

    held_keys::given(DECK, PLACER, NOTES_VIEW);
    assert_eq!(
        notes((7, 6), vec![Forwarded::Focus(true), letter("a")]),
        [Forwarded::Focus(true)],
        "until the notes' app has drawn a frame that took the focus, their own keys wait behind the held ones"
    );
    assert_eq!(
        held_keys::owed_now(TEXT, NOTES_EDITOR, NOTES_VIEW, false, 7),
        None,
        "the frame asked before the focus arrived does not count"
    );
    assert_eq!(
        held_keys::owed_now(TEXT, NOTES_EDITOR, NOTES_VIEW, false, 8),
        Some(Modifiers::NONE)
    );
    assert_eq!(
        notes((8, 8), Vec::new()),
        [letter("T"), letter("h"), letter("a")],
        "in the order they were typed"
    );
    assert_eq!(notes((8, 8), vec![letter("n")]), [letter("n")]);

    assert_eq!(deck(false, vec![letter("x")]).0, [letter("x")], "with no gift pending the deck hears its keys");

    assert_eq!(deck(true, vec![letter("y")]).0, []);
    held_keys::refused(DECK, PLACER);
    assert_eq!(held_keys::owed_now(DECK, PLACER, DECK_VIEW, false, 4), Some(Modifiers::NONE));
    assert_eq!(deck(false, Vec::new()).0, [letter("y")], "a gift that falls through gives the deck its keys back");

    assert_eq!(deck(true, vec![letter("z")]).0, []);
    let (events, routed) = deck(true, vec![Forwarded::Focus(false)]);
    assert!(routed.refuse, "taking the keyboard elsewhere ends the gift");
    assert_eq!(events, [letter("z"), Forwarded::Focus(false)], "and the deck hears its keys before it lets go");

    assert_eq!(deck(true, vec![letter("w")]).0, []);
    assert_eq!(
        held_keys::owed_now(DECK, PLACER, DECK_VIEW, false, 4),
        Some(Modifiers::NONE),
        "a gift that ends because the child went gives the keys back"
    );
    assert_eq!(deck(false, Vec::new()).0, [letter("w")]);
}

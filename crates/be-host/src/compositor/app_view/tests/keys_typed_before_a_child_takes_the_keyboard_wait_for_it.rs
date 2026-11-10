use beui::{Event, Forwarded, ForwardedInput, KeyPress, Modifiers};

use super::super::held_keys::{self, App, View};
use be_sdk_api::{EditorInstanceId, ViewId};

const DECK: (&str, EditorInstanceId) = ("be3.presentation", EditorInstanceId(6));
const TEXT: (&str, EditorInstanceId) = ("be3.text", EditorInstanceId(10));
fn deck_view() -> View {
    (DECK.0.to_owned(), DECK.1, ViewId::MAIN)
}

fn notes_view() -> View {
    (TEXT.0.to_owned(), TEXT.1, ViewId::MAIN)
}
const SETTLED: App = App {
    frames: (4, 4),
    content: true,
    typing: false,
};

fn input(focused: bool, events: Vec<Forwarded>) -> ForwardedInput {
    ForwardedInput {
        events,
        rect: beui::Rect::ZERO,
        hovered: false,
        focused,
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

fn deck(pending: bool, focused: bool, events: Vec<Forwarded>) -> (Vec<Forwarded>, bool) {
    let mut input = input(focused, events);
    let routed = held_keys::route(&deck_view(), pending, SETTLED, &mut input);
    (input.events, routed.refuse)
}

fn notes(app: App, events: Vec<Forwarded>) -> Vec<Forwarded> {
    let mut input = input(true, events);
    held_keys::route(&notes_view(), false, app, &mut input);
    input.events
}

#[test]
fn keys_typed_before_a_child_takes_the_keyboard_wait_for_it() {
    assert_eq!(
        deck(true, true, vec![letter("T")]).0,
        [],
        "the deck does not hear a key meant for its notes"
    );
    assert_eq!(deck(true, true, vec![letter("h")]).0, []);

    held_keys::given(DECK, notes_view(), false);
    let loading = App {
        frames: (7, 6),
        content: false,
        typing: false,
    };
    assert_eq!(
        notes(loading, vec![Forwarded::Focus(true), letter("a")]),
        [Forwarded::Focus(true)],
        "while the notes wait for their content, their own keys wait behind the held ones"
    );
    let loaded = App {
        frames: (9, 9),
        content: true,
        typing: false,
    };
    assert_eq!(
        held_keys::owed_now(&notes_view(), false, loaded, true),
        None,
        "having the content and the focus is not enough: the field has yet to take the keyboard"
    );
    let drawn = App {
        frames: (10, 10),
        content: true,
        typing: true,
    };
    assert_eq!(
        held_keys::owed_now(&notes_view(), false, drawn, true),
        Some(Modifiers::NONE)
    );
    assert_eq!(
        notes(drawn, Vec::new()),
        [letter("T"), letter("h"), letter("a")],
        "in the order they were typed"
    );
    assert_eq!(notes(drawn, vec![letter("n")]), [letter("n")]);

    held_keys::given(DECK, notes_view(), true);
    assert_eq!(
        notes(drawn, vec![letter("o")]),
        [letter("o")],
        "a gift to a child that already has the keyboard, with nothing held, leaves its typing alone"
    );

    assert_eq!(
        deck(false, true, vec![letter("x")]).0,
        [letter("x")],
        "with no gift pending the deck hears its keys"
    );

    assert_eq!(deck(true, true, vec![letter("y")]).0, []);
    held_keys::refused(DECK, |owner| *owner == deck_view());
    assert_eq!(
        held_keys::owed_now(&deck_view(), false, SETTLED, true),
        Some(Modifiers::NONE)
    );
    assert_eq!(
        deck(false, true, Vec::new()).0,
        [letter("y")],
        "a gift that falls through gives the deck its keys back"
    );

    assert_eq!(deck(true, true, vec![letter("z")]).0, []);
    let (events, refuse) = deck(true, false, vec![Forwarded::Focus(false)]);
    assert!(refuse, "taking the keyboard elsewhere ends the gift");
    assert_eq!(
        events,
        [letter("z"), Forwarded::Focus(false)],
        "and the deck hears its keys before it lets go"
    );

    assert_eq!(deck(true, true, vec![letter("w")]).0, []);
    held_keys::refused(DECK, |_| false);
    assert_eq!(
        held_keys::owed_now(&deck_view(), false, SETTLED, false),
        None,
        "keys given back to a deck that lost the keyboard are dropped"
    );
    assert_eq!(
        deck(false, true, Vec::new()).0,
        [],
        "and never replayed later"
    );

    assert_eq!(deck(true, true, vec![letter("v")]).0, []);
    assert_eq!(
        held_keys::owed_now(&deck_view(), false, SETTLED, true),
        Some(Modifiers::NONE),
        "a gift that ends because the child went gives the keys back"
    );
    assert_eq!(deck(false, true, Vec::new()).0, [letter("v")]);
}

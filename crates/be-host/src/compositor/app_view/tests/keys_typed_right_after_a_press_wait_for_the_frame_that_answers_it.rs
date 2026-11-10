use super::*;
use be_sdk_api::{EditorInstanceId, ViewId};
use beui::{KeyPress, PointerButton};

use super::super::held_keys::{self, App, View};

const DECK: (&str, EditorInstanceId) = ("be3.presentation", EditorInstanceId(16));
const TEXT: (&str, EditorInstanceId) = ("be3.text", EditorInstanceId(17));

fn deck_view() -> View {
    (DECK.0.to_owned(), DECK.1, ViewId::MAIN)
}

fn notes_view() -> View {
    (TEXT.0.to_owned(), TEXT.1, ViewId::MAIN)
}

fn press() -> Forwarded {
    Forwarded::Pointer {
        event: PointerEvent {
            pointer: PointerId::Mouse,
            window: beui::WindowId::MAIN,
            phase: PointerPhase::Down(PointerButton::Primary),
            pos: pos2(10.0, 10.0),
            modifiers: Default::default(),
            clicks: 1,
            force: None,
            pen: None,
            predicted: None,
            time: std::time::Instant::now(),
            source: 0,
        },
        over: None,
    }
}

fn letter(text: &str) -> Forwarded {
    Forwarded::Input(beui::Event::Key {
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

fn app(asked: u64, answered: u64) -> App {
    App {
        frames: (asked, answered),
        content: true,
        typing: false,
    }
}

fn deck(pending: bool, frames: App, events: Vec<Forwarded>) -> Vec<Forwarded> {
    let mut input = ForwardedInput {
        events,
        rect: Rect::ZERO,
        hovered: true,
        focused: true,
        pointer: None,
        modifiers: beui::Modifiers::NONE,
    };
    held_keys::route(&deck_view(), pending, frames, &mut input);
    input.events
}

#[test]
fn keys_typed_right_after_a_press_wait_for_the_frame_that_answers_it() {
    assert_eq!(
        deck(false, app(3, 3), vec![press()]).len(),
        1,
        "the press goes through"
    );
    assert_eq!(
        deck(false, app(4, 3), vec![letter("T")]),
        [],
        "a key typed before the deck drew the frame that took the press waits for it"
    );
    assert_eq!(
        deck(true, app(4, 4), vec![letter("h")]),
        [],
        "that frame asked to give the keyboard to the notes, so the keys are theirs"
    );
    held_keys::given(DECK, notes_view(), false);
    let mut notes = ForwardedInput {
        events: Vec::new(),
        rect: Rect::ZERO,
        hovered: false,
        focused: true,
        pointer: None,
        modifiers: beui::Modifiers::NONE,
    };
    let typing = App {
        frames: (1, 1),
        content: true,
        typing: true,
    };
    held_keys::route(&notes_view(), false, typing, &mut notes);
    assert_eq!(notes.events, [letter("T"), letter("h")]);

    assert_eq!(deck(false, app(5, 5), vec![press()]).len(), 1);
    assert_eq!(deck(false, app(6, 5), vec![letter("x")]), []);
    assert_eq!(
        held_keys::owed_now(&deck_view(), false, app(6, 6), true),
        Some(beui::Modifiers::NONE),
        "a press whose frame asked for nothing gives the deck its keys as soon as it is drawn"
    );
    assert_eq!(deck(false, app(6, 6), Vec::new()), [letter("x")]);
}

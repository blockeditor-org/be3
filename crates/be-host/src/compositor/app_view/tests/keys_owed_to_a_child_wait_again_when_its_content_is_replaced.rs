use super::*;
use be_sdk_api::{EditorInstanceId, ViewId};
use beui::KeyPress;

use super::super::held_keys::{self, App, View};

const DECK: (&str, EditorInstanceId) = ("be3.presentation", EditorInstanceId(26));

fn notes_view() -> View {
    ("be3.text".to_owned(), EditorInstanceId(27), ViewId::MAIN)
}

fn app(frames: (u64, u64), content: bool, typing: bool) -> App {
    App {
        frames,
        content,
        typing,
    }
}

#[test]
fn keys_owed_to_a_child_wait_again_when_its_content_is_replaced() {
    held_keys::given(DECK, notes_view(), false);
    let mut input = ForwardedInput {
        events: vec![Forwarded::Input(beui::Event::Key {
            press: KeyPress {
                key: beui::Key::A,
                pressed: true,
                repeat: false,
                modifiers: beui::Modifiers::NONE,
            },
            evdev: None,
            text: Some("T".to_owned()),
        })],
        rect: Rect::ZERO,
        hovered: false,
        focused: true,
        pointer: None,
        modifiers: beui::Modifiers::NONE,
    };
    held_keys::route(&notes_view(), false, app((0, 0), true, false), &mut input);
    assert!(
        input.events.is_empty(),
        "the child is not ready for its keys"
    );
    let owed = |app| held_keys::owed_now(&notes_view(), false, app, true);
    assert_eq!(owed(app((1, 1), true, false)), None);
    assert_eq!(
        owed(app((2, 2), false, false)),
        None,
        "two frames went by but the content went away meanwhile, so the editor has nowhere to \
         put keys yet"
    );
    assert_eq!(
        owed(app((3, 3), true, false)),
        None,
        "its frames count again"
    );
    assert_eq!(
        owed(app((4, 4), true, true)),
        Some(beui::Modifiers::NONE),
        "its text field has the keyboard"
    );
}

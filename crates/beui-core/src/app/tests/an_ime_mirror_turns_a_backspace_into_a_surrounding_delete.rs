use super::*;
use crate::app::ime_mirror::{ImeInput, ImeMirror};
use crate::input::{ImeEvent, ImeText};

#[test]
fn an_ime_mirror_turns_a_backspace_into_a_surrounding_delete() {
    let mut mirror = ImeMirror::default();
    mirror.sync(Some(&ImeText {
        start: 0,
        text: "aé|b".replace('|', ""),
        selection: 3..3,
        composing: None,
    }));
    let mut events = Vec::new();

    mirror.input(
        ImeInput {
            value: "ab",
            selection: (1, 1),
            composing: false,
            replacement: false,
        },
        &mut events,
    );

    assert_eq!(
        events,
        vec![Event::Ime(ImeEvent::DeleteSurrounding {
            before: 2,
            after: 0,
        })]
    );
}

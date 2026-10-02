use super::*;
use crate::app::ime_mirror::{ImeInput, ImeMirror};
use crate::input::{ImeEvent, ImeText};

#[test]
fn an_ime_mirror_turns_a_correction_into_a_replacement() {
    let mut mirror = ImeMirror::default();
    assert!(mirror.sync(Some(&ImeText {
        start: 10,
        text: "say helo ".to_owned(),
        selection: 19..19,
        composing: None,
    })));
    let mut events = Vec::new();

    mirror.input(
        ImeInput {
            value: "say hello ",
            selection: (10, 10),
            composing: false,
            replacement: true,
        },
        &mut events,
    );

    assert_eq!(
        events,
        vec![Event::Ime(ImeEvent::ReplaceText {
            range: 17..19,
            text: "lo ".to_owned(),
        })]
    );
}

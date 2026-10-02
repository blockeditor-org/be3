use super::*;
use crate::app::ime_mirror::{ImeInput, ImeMirror};
use crate::input::{ImeEvent, ImeText};

#[test]
fn an_ime_mirror_turns_composing_input_into_composing_text() {
    let mut mirror = ImeMirror::default();
    mirror.sync(Some(&ImeText {
        start: 0,
        text: "say ".to_owned(),
        selection: 4..4,
        composing: None,
    }));
    mirror.start_composition();
    let mut events = Vec::new();

    for value in ["say h", "say he", "say 你"] {
        let selection = value.encode_utf16().count() as u32;
        mirror.input(
            ImeInput {
                value,
                selection: (selection, selection),
                composing: true,
                replacement: false,
            },
            &mut events,
        );
    }
    mirror.end_composition(&mut events);

    assert_eq!(
        events,
        vec![
            Event::Ime(ImeEvent::SetComposingText("h".to_owned())),
            Event::Ime(ImeEvent::SetComposingText("he".to_owned())),
            Event::Ime(ImeEvent::SetComposingText("你".to_owned())),
            Event::Ime(ImeEvent::FinishComposing),
        ]
    );
}

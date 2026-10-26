use super::*;
use crate::input::ImeEvent;
use crate::reactive::*;
use crate::unstyled::{TextArea, TextAreaState};
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn an_ime_composition_pushes_the_text_after_the_caret_along() {
    let _root = crate::reactive::RootScope::new();
    let held = create_signal::<Option<TextAreaState>>(None);
    let document = build_node(move || {
        let document =
            Arc::new(TextBuffer::new(b"hello world")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        held.update(|held| *held = Some(state));
        view! {
            <TextArea
                colors={area_colors()}
                font_size=AREA_FONT_SIZE
                padding=AREA_PADDING
                state={state}
                single_line=true
            />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let state = held.get_untracked().expect("the text area was built");
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Home, Modifiers::NONE);
    harness.key(Key::ArrowRight, Modifiers::NONE);
    let x_of = |byte: usize| {
        state
            .layout()
            .get_untracked()
            .caret_rect(byte)
            .expect("every byte of the text has a place")
            .min
            .x
    };
    let before = [1, 5, 11].map(x_of);
    let first = x_of(0);

    harness.frame(vec![Input::Ime(ImeEvent::SetComposingText(
        "xyz".to_owned(),
    ))]);
    let composing = [4, 8, 14].map(x_of);
    assert_eq!(
        String::from_utf8_lossy(&state.bytes()),
        "hxyzello world",
        "the composition is written where the caret was"
    );
    for (was, is) in before.iter().zip(composing) {
        assert!(
            is > *was,
            "the text after the caret moves along for the composition: {before:?} then {composing:?}"
        );
    }
    assert_eq!(
        x_of(0),
        first,
        "the text before the caret stays where it was"
    );

    harness.frame(vec![Input::Ime(ImeEvent::SetComposingText(String::new()))]);
    assert_eq!(
        [1, 5, 11].map(x_of),
        before,
        "a cancelled composition gives the text its place back"
    );
}

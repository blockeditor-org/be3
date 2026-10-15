use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::TextAreaState;
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn pressing_enter_at_the_end_of_a_text_area_takes_the_caret_off_the_line_it_left() {
    let _root = crate::reactive::RootScope::new();
    let (held, set_held) = create_signal::<Option<TextAreaState>>(None);
    let document = build(move || {
        let document = Arc::new(TextBuffer::new(b"")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        set_held.update(|held| *held = Some(state));
        view! {
            <TextArea state={state} />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let state = held.get_untracked().expect("the text area was built");

    harness.click(pos2(300.0, 16.0));
    harness.type_text("abcd");
    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(state.caret_indices(), vec![5]);
    let document = harness.document();
    let root = document.root().expect("the document has a root");
    let first = document
        .texts_within(root)
        .into_iter()
        .find(|text| document.text(*text).contains("abcd"))
        .expect("the first line is laid out");
    assert_eq!(
        document.text_carets(first),
        &[],
        "the line the caret left paints no caret"
    );
}

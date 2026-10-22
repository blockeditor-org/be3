use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn dragging_the_end_handle_of_a_double_tapped_word_extends_the_selection() {
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <TextInput value="Hello world" />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let text = harness.rect(
        harness
            .document()
            .api::<unstyled::TextInputApi>(input)
            .text(),
    );
    let word = pos2(text.left() + 10.0, text.center().y);
    for _ in 0..2 {
        harness.touch(TouchPhase::Start, word);
        harness.touch(TouchPhase::End, word);
    }
    harness.frame(Vec::new());
    assert_eq!(
        harness
            .document()
            .api::<unstyled::TextInputApi>(input)
            .selection()
            .first(),
        Some(&(0..5))
    );

    let handles = harness
        .document()
        .api::<unstyled::TextInputApi>(input)
        .handles(harness.document());
    let [_, end] = handles[..] else {
        panic!("a touch selection shows two handles, not {handles:?}");
    };
    let past_the_text = pos2(text.right() - 10.0, end.y);
    harness.touch(TouchPhase::Start, end);
    harness.touch(TouchPhase::Move, past_the_text);
    harness.touch(TouchPhase::End, past_the_text);
    harness.frame(Vec::new());

    assert_eq!(
        harness
            .document()
            .api::<unstyled::TextInputApi>(input)
            .selection()
            .first(),
        Some(&(0..11))
    );
    assert!(
        harness
            .document()
            .api::<unstyled::TextInputApi>(input)
            .focused()
            .get_untracked()
    );
}

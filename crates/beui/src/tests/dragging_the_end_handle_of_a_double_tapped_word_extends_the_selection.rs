use super::*;
use crate::reactive::view;
use crate::styled::TextInput;

#[test]
fn dragging_the_end_handle_of_a_double_tapped_word_extends_the_selection() {
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <TextInput value="Hello world" />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let text = harness.rect(unstyled::text_input_text(harness.document(), input));
    let word = pos2(text.left() + 10.0, text.center().y);
    for _ in 0..2 {
        harness.touch(TouchPhase::Start, word);
        harness.touch(TouchPhase::End, word);
    }
    harness.frame(Vec::new());
    assert_eq!(
        unstyled::text_input_selection(harness.document(), input).first(),
        Some(&(0..5))
    );

    let handles = unstyled::text_input_handles(harness.document(), input);
    let [_, end] = handles[..] else {
        panic!("a touch selection shows two handles, not {handles:?}");
    };
    let past_the_text = pos2(text.right() - 10.0, end.y);
    harness.touch(TouchPhase::Start, end);
    harness.touch(TouchPhase::Move, past_the_text);
    harness.touch(TouchPhase::End, past_the_text);
    harness.frame(Vec::new());

    assert_eq!(
        unstyled::text_input_selection(harness.document(), input).first(),
        Some(&(0..11))
    );
    assert!(unstyled::text_input_focused(harness.document(), input).get_untracked());
}

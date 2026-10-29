use super::*;
use crate::styled::*;

#[test]
fn touch_dragging_across_a_text_input_does_not_select_its_text() {
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
    let from = pos2(text.left() + 4.0, text.center().y);
    let to = pos2(text.left() + 60.0, text.center().y);

    harness.touch(TouchPhase::Start, from);
    harness.touch(TouchPhase::Move, to);
    harness.touch(TouchPhase::End, to);
    harness.frame(Vec::new());

    assert!(
        harness
            .document()
            .api::<unstyled::TextInputApi>(input)
            .selection()
            .is_empty()
    );
    assert_eq!(
        harness
            .document()
            .api::<styled::TextInputApi>(input)
            .value(),
        "Hello world"
    );
}

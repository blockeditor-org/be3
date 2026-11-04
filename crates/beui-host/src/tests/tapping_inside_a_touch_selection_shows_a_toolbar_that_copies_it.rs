use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn tapping_inside_a_touch_selection_shows_a_toolbar_that_copies_it() {
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
    let inside = pos2(text.left() + 24.0, text.center().y);
    harness.touch(TouchPhase::Start, inside);
    harness.touch(TouchPhase::End, inside);
    harness.frame(Vec::new());

    let toolbar = harness
        .document()
        .api::<unstyled::TextInputApi>(input)
        .toolbar(harness.document())
        .expect("tapping the selection shows its toolbar");
    assert!(
        harness
            .document()
            .api::<unstyled::TextInputApi>(input)
            .menu_row(harness.document(), 0)
            .is_none(),
        "the toolbar stands in for the menu"
    );
    let copy = text_within(harness.document(), toolbar, "Copy").expect("the toolbar copies");
    let row = harness.center(copy);
    harness.touch(TouchPhase::Start, row);
    let output = harness.touch(TouchPhase::End, row);

    assert_eq!(output.copied_text.as_deref().map(str::trim), Some("Hello"));
    assert!(
        harness
            .document()
            .api::<unstyled::TextInputApi>(input)
            .focused()
            .get_untracked()
    );
    assert_eq!(
        harness
            .document()
            .api::<unstyled::TextInputApi>(input)
            .selection()
            .first(),
        Some(&(0..5))
    );
}

use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn tapping_inside_a_selection_in_a_select_search_box_shows_its_toolbar() {
    let (document, [select]) = toolbar_of(|| {
        let options = view! {
            <unstyled::ChoiceOption value="Apple" label="Apple" />
            <unstyled::ChoiceOption value="Banana" label="Banana" />
        };
        [view! {
            <Select options selected=None />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let trigger = harness
        .document()
        .api::<unstyled::SelectApi<&str>>(select)
        .trigger();
    harness.click(harness.center(trigger));
    harness.frame(Vec::new());
    harness.type_text("apple pie");
    harness.frame(Vec::new());

    let search = harness
        .document()
        .api::<unstyled::SelectApi<&str>>(select)
        .search();
    let text = harness.rect(
        harness
            .document()
            .api::<unstyled::TextInputApi>(search)
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
        .api::<unstyled::TextInputApi>(search)
        .toolbar(harness.document())
        .expect("tapping the selection shows the search box's toolbar");
    assert!(text_within(harness.document(), toolbar, "Copy").is_some());
    assert!(
        harness
            .document()
            .api::<unstyled::SelectApi<&str>>(select)
            .open()
    );
    assert!(
        harness
            .document()
            .api::<unstyled::TextInputApi>(search)
            .focused()
            .get_untracked()
    );
}

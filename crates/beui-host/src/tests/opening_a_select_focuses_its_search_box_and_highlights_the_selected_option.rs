use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn opening_a_select_focuses_its_search_box_and_highlights_the_selected_option() {
    let (document, [select]) = toolbar_of(|| {
        let options = view! {
            <unstyled::ChoiceOption label="Apple" />
            <unstyled::ChoiceOption label="Banana" />
            <unstyled::ChoiceOption label="Cherry" />
        };
        [view! {
            <Select options selected=Some(1) />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let trigger = harness
        .document()
        .api::<unstyled::SelectApi>(select)
        .trigger();
    harness.click(harness.center(trigger));
    harness.frame(Vec::new());

    assert!(harness.document().api::<styled::SelectApi>(select).open());
    let search = harness
        .document()
        .api::<unstyled::SelectApi>(select)
        .search();
    assert!(
        harness
            .document()
            .api::<unstyled::TextInputApi>(search)
            .focused()
            .get()
    );
    assert_eq!(
        harness
            .document()
            .api::<unstyled::SelectApi>(select)
            .highlighted(),
        Some(1)
    );
}

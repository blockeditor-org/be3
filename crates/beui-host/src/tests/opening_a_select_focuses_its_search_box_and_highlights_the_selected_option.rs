use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn opening_a_select_focuses_its_search_box_and_highlights_the_selected_option() {
    let (document, [select]) = toolbar_of(|| {
        let options = view! {
            <unstyled::ChoiceOption value="Apple" label="Apple" />
            <unstyled::ChoiceOption value="Banana" label="Banana" />
            <unstyled::ChoiceOption value="Cherry" label="Cherry" />
        };
        [view! {
            <Select options selected=Some("Banana") />
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

    assert!(
        harness
            .document()
            .api::<styled::SelectApi<&str>>(select)
            .open()
    );
    let search = harness
        .document()
        .api::<unstyled::SelectApi<&str>>(select)
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
            .api::<unstyled::SelectApi<&str>>(select)
            .highlighted(),
        Some("Banana")
    );
}

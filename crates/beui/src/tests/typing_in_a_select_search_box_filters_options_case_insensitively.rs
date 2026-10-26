use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn typing_in_a_select_search_box_filters_options_case_insensitively() {
    let (document, [select]) = toolbar_of(|| {
        let options = view! {
            <unstyled::ChoiceOption label="Apple" />
            <unstyled::ChoiceOption label="Banana" />
            <unstyled::ChoiceOption label="Grape" />
        };
        [view! {
            <Select options selected=None />
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

    harness.type_text("BAN");
    harness.frame(Vec::new());

    let apple = harness
        .document()
        .api::<unstyled::SelectApi>(select)
        .option_button(0);
    let banana = harness
        .document()
        .api::<unstyled::SelectApi>(select)
        .option_button(1);
    let grape = harness
        .document()
        .api::<unstyled::SelectApi>(select)
        .option_button(2);
    assert!(apple.is_none());
    assert!(banana.is_some_and(|banana| harness.document().node_rect(banana).is_some()));
    assert!(grape.is_none());
    assert_eq!(
        harness
            .document()
            .api::<unstyled::SelectApi>(select)
            .highlighted(),
        Some(1)
    );
}

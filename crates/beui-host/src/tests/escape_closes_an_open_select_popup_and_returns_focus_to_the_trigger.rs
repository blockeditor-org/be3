use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn escape_closes_an_open_select_popup_and_returns_focus_to_the_trigger() {
    let (document, [select]) = toolbar_of(|| {
        let options = view! {
            <unstyled::ChoiceOption label="Apple" />
            <unstyled::ChoiceOption label="Banana" />
        };
        [view! {
            <Select options selected=Some(0) />
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

    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(Vec::new());

    assert!(!harness.document().api::<styled::SelectApi>(select).open());
    assert_eq!(
        harness
            .document()
            .api::<styled::SelectApi>(select)
            .selected(),
        Some(0)
    );
    assert!(
        harness
            .document()
            .api::<unstyled::ButtonApi>(trigger)
            .focused()
            .get()
    );
}

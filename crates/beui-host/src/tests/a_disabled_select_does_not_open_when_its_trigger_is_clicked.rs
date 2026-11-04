use super::*;
use crate::styled::*;

#[test]
fn a_disabled_select_does_not_open_when_its_trigger_is_clicked() {
    let (document, [select]) = toolbar_of(|| {
        let options = view! {
            <unstyled::ChoiceOption label="String" />
            <unstyled::ChoiceOption label="Number" />
        };
        [view! {
            <Select options selected=Some(0) disabled=true />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.click(harness.center(select));
    harness.frame(Vec::new());

    assert!(!harness.document().api::<unstyled::SelectApi>(select).open());
    assert_eq!(
        harness
            .document()
            .api::<unstyled::SelectApi>(select)
            .selected(),
        Some(0)
    );
}

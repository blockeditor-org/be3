use super::*;
use crate::reactive::view;
use crate::styled::Select;

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

    assert!(!unstyled::select_open(harness.document(), select));
    assert_eq!(
        unstyled::select_selected(harness.document(), select),
        Some(0)
    );
}

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

    let trigger = unstyled::select_trigger(harness.document(), select);
    harness.click(harness.center(trigger));
    harness.frame(Vec::new());
    assert!(styled::select_open(harness.document(), select));

    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(Vec::new());

    assert!(!styled::select_open(harness.document(), select));
    assert_eq!(styled::select_selected(harness.document(), select), Some(0));
    assert!(
        unstyled::button_state(harness.document(), trigger)
            .get()
            .focused
    );
}

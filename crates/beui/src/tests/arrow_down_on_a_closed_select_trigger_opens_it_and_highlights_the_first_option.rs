use super::*;
use crate::reactive::view;
use crate::styled::Select;

#[test]
fn arrow_down_on_a_closed_select_trigger_opens_it_and_highlights_the_first_option() {
    let (document, [select]) = toolbar_of(|| {
        let options = view! {
            <unstyled::ChoiceOption label="Apple" />
            <unstyled::ChoiceOption label="Banana" />
            <unstyled::ChoiceOption label="Cherry" />
        };
        [view! {
            <Select options selected=None />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let inner = select;
    let trigger = unstyled::select_trigger(harness.document(), inner);
    with_installed(harness.document_mut(), |_| {
        crate::focus_within(trigger);
    });
    harness.frame(Vec::new());

    assert!(!unstyled::select_open(harness.document(), inner));

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());

    assert!(unstyled::select_open(harness.document(), inner));
    assert_eq!(
        unstyled::select_highlighted(harness.document(), inner),
        Some(0)
    );
}

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

    let trigger = unstyled::select_trigger(harness.document(), select);
    harness.click(harness.center(trigger));
    harness.frame(Vec::new());

    harness.type_text("BAN");
    harness.frame(Vec::new());

    let apple = unstyled::select_option_button(harness.document(), select, 0);
    let banana = unstyled::select_option_button(harness.document(), select, 1);
    let grape = unstyled::select_option_button(harness.document(), select, 2);
    assert!(harness.document().node_rect(apple).is_none());
    assert!(harness.document().node_rect(banana).is_some());
    assert!(harness.document().node_rect(grape).is_none());
    assert_eq!(
        unstyled::select_highlighted(harness.document(), select),
        Some(1)
    );
}

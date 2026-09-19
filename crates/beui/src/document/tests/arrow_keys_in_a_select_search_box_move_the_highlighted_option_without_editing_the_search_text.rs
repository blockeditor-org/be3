use super::*;
use crate::reactive::view;
use crate::styled::Select;

#[test]
fn arrow_keys_in_a_select_search_box_move_the_highlighted_option_without_editing_the_search_text() {
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
    harness.click(harness.center(trigger));
    harness.frame(Vec::new());
    let search = unstyled::select_search(harness.document(), inner);

    assert_eq!(
        unstyled::select_highlighted(harness.document(), inner),
        None
    );

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(
        unstyled::select_highlighted(harness.document(), inner),
        Some(0)
    );
    assert_eq!(unstyled::text_input_value(harness.document(), search), "");

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(
        unstyled::select_highlighted(harness.document(), inner),
        Some(1)
    );
    assert_eq!(unstyled::text_input_value(harness.document(), search), "");

    harness.key(Key::ArrowUp, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(
        unstyled::select_highlighted(harness.document(), inner),
        Some(0)
    );
}

use super::*;
use crate::reactive::*;

#[test]
fn tab_moves_focus_to_the_next_button() {
    let (document, [first, second]) = toolbar_of(|| {
        [
            view! {
                <LabelledButton label="First" />
            },
            view! {
                <LabelledButton label="Second" />
            },
        ]
    });
    let first_focused = unstyled::button_state(&document, first);
    let second_focused = unstyled::button_state(&document, second);
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    assert_eq!(
        (first_focused.get().focused, second_focused.get().focused),
        (true, false)
    );

    harness.key(Key::Tab, Modifiers::NONE);
    assert_eq!(
        (first_focused.get().focused, second_focused.get().focused),
        (false, true)
    );
}

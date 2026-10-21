use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn enter_confirms_the_highlighted_select_option_and_closes_the_popup() {
    let _root = crate::reactive::RootScope::new();
    let changes = create_signal(Vec::new());
    let (document, [select]) = toolbar_of(|| {
        let options = view! {
            <unstyled::ChoiceOption label="Apple" />
            <unstyled::ChoiceOption label="Banana" />
        };
        [view! {
            <Select
                options
                selected={Binding::new(None, move |selected| {
                    changes.update(|changes| changes.push(selected));
                })}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let trigger = unstyled::select_trigger(harness.document(), select);
    harness.click(harness.center(trigger));
    harness.frame(Vec::new());

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());
    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(styled::select_selected(harness.document(), select), Some(0));
    assert!(!styled::select_open(harness.document(), select));
    assert_eq!(changes.get_untracked().as_slice(), &[Some(0)]);
    assert!(unstyled::button_state(harness.document(), trigger).get().focused);
}

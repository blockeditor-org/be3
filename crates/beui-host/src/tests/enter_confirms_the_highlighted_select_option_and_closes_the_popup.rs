use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn enter_confirms_the_highlighted_select_option_and_closes_the_popup() {
    let _root = crate::reactive::RootScope::new();
    let changes = create_signal(Vec::new());
    let (document, [select]) = toolbar_of(|| {
        let options = view! {
            <unstyled::ChoiceOption value="Apple" label="Apple" />
            <unstyled::ChoiceOption value="Banana" label="Banana" />
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

    let trigger = harness
        .document()
        .api::<unstyled::SelectApi<&str>>(select)
        .trigger();
    harness.click(harness.center(trigger));
    harness.frame(Vec::new());

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());
    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(
        harness
            .document()
            .api::<styled::SelectApi<&str>>(select)
            .selected(),
        Some("Apple")
    );
    assert!(
        !harness
            .document()
            .api::<styled::SelectApi<&str>>(select)
            .open()
    );
    assert_eq!(changes.get_untracked().as_slice(), &[Some("Apple")]);
    assert!(
        harness
            .document()
            .api::<unstyled::ButtonApi>(trigger)
            .focused()
            .get()
    );
}

use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn accordion_headers_are_keyboard_operable_and_skip_collapsed_content() {
    let _root = crate::reactive::RootScope::new();
    let child = NodeRef::new();
    let (document, [accordion, after]) = toolbar_of(move || {
        [
            view! {
                <Accordion title="Options" open=false>
                    <LabelledButton @node_ref=&child label="Child" />
                </Accordion>
            },
            view! {
                <LabelledButton label="After" />
            },
        ]
    });
    let child_focus = document.api::<unstyled::ButtonApi>(child.get()).focused();
    let after_focus = document.api::<unstyled::ButtonApi>(after).focused();
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    assert!(after_focus.get());
    assert!(!child_focus.get());
    harness.key(Key::Tab, Modifiers::SHIFT);
    harness.key(Key::Enter, Modifiers::NONE);
    assert!(
        harness
            .document()
            .api::<styled::DisclosureApi>(accordion)
            .open()
    );
    harness.key(Key::Tab, Modifiers::NONE);
    assert!(child_focus.get());
    harness.key(Key::Tab, Modifiers::SHIFT);
    harness.key(Key::Space, Modifiers::NONE);
    assert!(
        !harness
            .document()
            .api::<styled::DisclosureApi>(accordion)
            .open()
    );
}

use accesskit::Role;

use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_disabled_text_input_ignores_typing_and_reads_as_dimmed() {
    let _root = crate::reactive::RootScope::new();
    let changes = create_signal(Vec::new());
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <TextInput
                value={Binding::new("Locked".to_owned(), move |next| changes.update(|changes| changes.push(next)))}
                disabled=true
            />
        }]
    });
    let mut harness = Harness::new(document);
    let output = harness.frame(Vec::new());

    let tree = output.accessibility_tree("Test", VIEWPORT);
    let field = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::TextInput)
        .expect("the field is absent from the accessibility tree");
    assert!(field.1.is_disabled());

    harness.key(Key::Tab, Modifiers::NONE);
    harness.type_text("a");
    harness.click(harness.center(input));
    harness.type_text("b");
    harness.frame(Vec::new());

    assert_eq!(
        harness
            .document()
            .api::<unstyled::TextInputApi>(input)
            .shown(harness.document()),
        "Locked"
    );
    assert!(changes.with_untracked(|changes| changes.is_empty()));
}

use accesskit::Role;

use super::*;
use crate::reactive::view;
use crate::styled::TextInput;

#[test]
fn a_disabled_text_input_ignores_typing_and_reads_as_dimmed() {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let sink = changes.clone();
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <TextInput
                value="Locked"
                disabled=true
                on_change={move |value| sink.borrow_mut().push(value)}
            />
        }]
    });
    let mut harness = Harness::new(document);
    let output = harness.frame(Vec::new());
    let text = unstyled::text_input_text(harness.document(), input);

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

    assert_eq!(text_of(harness.document(), text), "Locked");
    assert!(changes.borrow().is_empty());
}

use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn undo_in_a_text_input_leaves_a_value_set_from_outside() {
    let _root = crate::reactive::RootScope::new();
    let (value, set_value) = create_signal("Hello".to_string());
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <TextInput value />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let rect = harness.rect(input);
    harness.click(pos2(rect.right() - 1.0, rect.center().y));
    harness.type_text("!");
    with_installed(harness.document_mut(), |_| {
        set_value.set("Oh, Hello!".to_string());
    });
    harness.frame(Vec::new());

    harness.key(Key::Z, Modifiers::CTRL);
    assert_eq!(
        styled::text_input_value(harness.document(), input),
        "Oh, Hello",
        "undo takes back what was typed, not the value set since"
    );
    harness.key(Key::Z, Modifiers::CTRL);
    assert_eq!(
        styled::text_input_value(harness.document(), input),
        "Oh, Hello"
    );
}

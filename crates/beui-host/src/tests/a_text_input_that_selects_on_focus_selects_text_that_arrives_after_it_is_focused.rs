use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_text_input_that_selects_on_focus_selects_text_that_arrives_after_it_is_focused() {
    let _root = crate::reactive::RootScope::new();
    let field = NodeRef::new();
    let name = create_signal(String::new());
    let (document, [_]) = toolbar_of(move || {
        let built = view! {
            <TextInput @node_ref=&field value={name} label="Name" select_on_focus=true />
        };
        create_effect(move || {
            if let Some(node) = field.try_get() {
                node.focus_into();
            }
        });
        [built]
    });
    let mut harness = Harness::new(document);
    harness.settle();
    name.set("Notes".to_owned());
    harness.settle();
    let input = harness
        .document()
        .api::<unstyled::TextInputApi>(field.get());
    assert_eq!(
        input.selection(),
        vec![0..5],
        "a name that arrives while the field still holds nothing typed is selected"
    );

    harness.frame(vec![Input::Text("Jour".to_owned())]);
    name.set("Notes".to_owned());
    harness.settle();
    let input = harness
        .document()
        .api::<unstyled::TextInputApi>(field.get());
    assert!(
        input.selection().is_empty(),
        "once something is typed the field keeps the caret where the typing left it"
    );
}

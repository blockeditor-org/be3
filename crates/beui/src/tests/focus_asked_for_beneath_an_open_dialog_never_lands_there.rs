use super::*;
use crate::primitives::Frame;
use crate::reactive::*;
use crate::styled::*;
use crate::primitives::List;

#[test]
fn focus_asked_for_beneath_an_open_dialog_never_lands_there() {
    let _root = crate::reactive::RootScope::new();
    let changes = create_signal(Vec::new());
    let (outside, field) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <List spacing=8.0>
                <Interactive
                    @node_ref=&outside
                    focusable=true
                    on_focus_change={move |focused: bool| changes.update(|changes| changes.push(focused))}
                >
                    <Frame />
                </Interactive>
                <Dialog open=true title="Rename" on_dismiss={move || {}}>
                    <TextInput @node_ref=&field value="" focused=true />
                </Dialog>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert!(harness.document().focus_is_within(field.get()));

    harness.document_mut().focus_focusable(outside.get());
    harness.frame(Vec::new());
    assert!(
        harness.document().focus_is_within(field.get()),
        "the dialog keeps the focus"
    );
    assert!(
        changes.get_untracked().is_empty(),
        "the control beneath the dialog never heard that it had the focus"
    );
}

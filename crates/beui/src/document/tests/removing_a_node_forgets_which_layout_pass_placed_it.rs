use super::*;
use crate::reactive::{Text, view};

#[test]
fn removing_a_node_forgets_which_layout_pass_placed_it() {
    let (document, [label]) = toolbar_of(|| {
        [view! {
            <Text string="hi" />
        }]
    });
    let list = document.root().expect("the toolbar is the root");
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert!(
        harness.document().placed_pass.contains_key(&label)
            && harness.document().reached_pass.contains_key(&label),
        "laying the node out records the pass that placed it"
    );

    harness.document_mut().remove_child(list, label);
    harness.document_mut().remove_node(label);
    harness.frame(Vec::new());

    assert!(
        !harness.document().placed_pass.contains_key(&label),
        "a removed node leaves nothing behind in the passes that placed it"
    );
    assert!(
        !harness.document().reached_pass.contains_key(&label),
        "a removed node leaves nothing behind in the passes that reached it"
    );
}

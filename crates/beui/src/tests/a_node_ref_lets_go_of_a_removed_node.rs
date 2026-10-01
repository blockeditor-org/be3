use super::*;
use crate::reactive::{NodeRef, view};
use crate::styled::Caption;

#[test]
fn a_node_ref_lets_go_of_a_removed_node() {
    let caption = NodeRef::new();
    let filled = caption.clone();
    let (document, [built]) = toolbar_of(move || {
        [view! {
            <Caption @node_ref=&filled content="Caption" />
        }]
    });
    let list = kind_of::<ListNode>(&document, document.root().expect("the toolbar is the root"));
    assert_eq!(caption.try_get(), Some(built));

    let mut harness = Harness::new(document);
    harness.document_mut().remove_child(list, built);
    harness.document_mut().remove_node(built);
    assert_eq!(caption.try_get(), None);
}

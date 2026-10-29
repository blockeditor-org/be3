use super::*;
use crate::reactive::*;
use crate::styled::Text;
use crate::styled::*;

#[test]
fn a_node_ref_lets_go_of_a_removed_node() {
    let _root = crate::reactive::RootScope::new();
    let caption = NodeRef::new();
    let (document, [built]) = toolbar_of(move || {
        [view! {
            <Text style=TextStyle::CAPTION @node_ref=&caption>"Caption"</Text>
        }]
    });
    let list = kind_of::<ListNode>(&document, document.root().expect("the toolbar is the root"));
    assert_eq!(caption.try_get(), Some(built));

    let mut harness = Harness::new(document);
    harness.document_mut().remove_child(list, built);
    harness.document_mut().remove_node(built);
    assert_eq!(caption.try_get(), None);
}

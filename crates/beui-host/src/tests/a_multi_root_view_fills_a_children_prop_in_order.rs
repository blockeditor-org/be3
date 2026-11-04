use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_multi_root_view_fills_a_children_prop_in_order() {
    let _root = crate::reactive::RootScope::new();
    let column = NodeRef::new();
    let (first, second, third) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        let toolbar = view! {
            <Text @node_ref=&first color=Color32::WHITE>"One"</Text>
            <Text @node_ref=&second color=Color32::WHITE>"Two"</Text>
            {view! {
                <Text @node_ref=&third color=Color32::WHITE>"Three"</Text>
            }}
        };
        view! {
            <List @node_ref=&column spacing=0.0 children={toolbar} />
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(
        harness.document().children(column.get()),
        vec![first.get(), second.get(), third.get()],
        "a multi-root view! must hand its roots to the children prop in source order"
    );
    assert_eq!(text_of(harness.document(), first.get()), "One");
    assert_eq!(text_of(harness.document(), third.get()), "Three");
}

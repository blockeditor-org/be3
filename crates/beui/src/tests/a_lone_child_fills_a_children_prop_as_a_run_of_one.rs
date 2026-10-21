use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_lone_child_fills_a_children_prop_as_a_run_of_one() {
    let _root = crate::reactive::RootScope::new();
    let column = NodeRef::new();
    let only = NodeRef::new();
    let document = build_node(move || {
        let lone = view! {
            <Text @node_ref=&only color=Color32::WHITE>"One"</Text>
        };
        view! {
            <List @node_ref=&column spacing=0.0 children={lone} />
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(
        harness.document().children(column.get()),
        vec![only.get()],
        "a single-root view! must fill a children prop as a run of one child"
    );
}

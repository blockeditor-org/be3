use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn list_items_at_the_roots_of_a_multi_root_view_are_honoured() {
    let _root = crate::reactive::RootScope::new();
    let (left, right) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        let panes = view! {
            <ListItem length=30.0>
                <List @node_ref=&left spacing=0.0></List>
            </ListItem>
            <ListItem grow=1.0>
                <List @node_ref=&right spacing=0.0></List>
            </ListItem>
        };
        view! {
            <List direction=Direction::Horizontal spacing=0.0 children={panes} />
        }
    });

    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    assert_eq!(harness.rect(left.get()).width(), 30.0);
    assert_eq!(
        harness.rect(right.get()).width(),
        WIDE_VIEWPORT.x - 30.0,
        "a `ListItem` at a fragment root must reach the row that takes the fragment"
    );
}

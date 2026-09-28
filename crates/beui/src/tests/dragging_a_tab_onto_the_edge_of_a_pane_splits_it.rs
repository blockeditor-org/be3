use super::*;
use crate::unstyled::{Entry, TabId, dock_state};

#[test]
fn dragging_a_tab_onto_the_edge_of_a_pane_splits_it() {
    let (document, dock) = dock_of(2);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let tab = dock_tab(harness.document(), dock, "Tab 2");
    let from = harness.center(tab);

    harness.drag(from, pos2(WIDE_VIEWPORT.x - 20.0, WIDE_VIEWPORT.y / 2.0));
    harness.frame(Vec::new());

    let state = dock_state(harness.document(), dock);
    let leaves = state.leaves(state.main());
    assert_eq!(
        leaves.len(),
        2,
        "a tab dropped on the edge of a pane splits it in two"
    );
    assert_eq!(
        state.entries(leaves[0]),
        vec![Entry::Tab(TabId::new(1))],
        "the tab that was left behind keeps the pane it was in"
    );
    assert_eq!(
        state.entries(leaves[1]),
        vec![Entry::Tab(TabId::new(2))],
        "the dropped tab lands in the pane the split made"
    );
    let first = harness.rect(harness.find("content.1"));
    let second = harness.rect(harness.find("content.2"));
    assert!(
        first.right() <= second.left(),
        "the pane made on the right edge is laid out to the right of the one it split"
    );
}

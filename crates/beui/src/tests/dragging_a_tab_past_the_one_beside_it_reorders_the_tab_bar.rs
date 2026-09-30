use super::*;
use crate::unstyled::{Entry, TabId, dock_state};

#[test]
fn dragging_a_tab_past_the_one_beside_it_reorders_the_tab_bar() {
    let (document, dock) = dock_of(2);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let first = harness.center(dock_tab(harness.document(), dock, "Tab 1"));
    let second = harness.rect(dock_tab(harness.document(), dock, "Tab 2"));

    harness.drag(first, pos2(second.right() + 4.0, second.center().y));
    harness.frame(Vec::new());

    let state = dock_state(harness.document(), dock);
    let leaf = state.leaves(state.main())[0];
    assert_eq!(
        state.entries(leaf),
        vec![Entry::Tab(TabId::new(2)), Entry::Tab(TabId::new(1))],
        "a tab dropped past the one beside it takes the place after it"
    );
    assert_eq!(
        state.active_tab(leaf),
        Some(TabId::new(1)),
        "the dropped tab is the one the pane shows"
    );
}

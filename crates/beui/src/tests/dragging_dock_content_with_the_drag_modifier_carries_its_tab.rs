use super::*;
use crate::unstyled::{Entry, TabId, dock_state};

#[test]
fn dragging_dock_content_with_the_drag_modifier_carries_its_tab() {
    let SplitDock {
        document,
        dock,
        presses,
    } = split_dock(Some(Modifiers::LOGO));
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let content = harness.center(harness.find("content.1"));
    let right = harness.center(harness.find("content.2"));

    drag_with(&mut harness, content, right, Modifiers::LOGO);
    harness.frame(Vec::new());

    let state = dock_state(harness.document(), dock);
    let leaves = state.leaves(state.main());
    assert_eq!(
        leaves.len(),
        1,
        "the tab dropped on the middle of the other pane joined it, emptying its own"
    );
    let entries = state.entries(leaves[0]);
    assert_eq!(entries.len(), 3);
    assert!(
        entries.contains(&Entry::Tab(TabId::new(1))),
        "the pane holds the tab that was carried into it: {entries:?}"
    );
    assert_eq!(
        presses.get(),
        0,
        "the press that picked the tab up never reached its content"
    );
}

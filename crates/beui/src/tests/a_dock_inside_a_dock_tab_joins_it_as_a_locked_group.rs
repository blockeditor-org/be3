use super::*;
use crate::dock::unstyled::{DockHome, Entry, TabId};

#[test]
fn a_dock_inside_a_dock_tab_joins_it_as_a_locked_group() {
    let _root = crate::reactive::RootScope::new();
    let (document, nested) = nested_dock();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    let state = nested.outer.state().get_untracked();
    assert!(
        !state.contains(TabId::new(2)),
        "the tab holding the inner dock gives way to it"
    );
    let group = state
        .find_group("joined:2")
        .expect("the inner dock is a group of the outer dock");
    assert!(state.is_pinned(group), "the group is locked");
    let main = state.leaves(state.main())[1];
    assert_eq!(
        state.entries(main),
        vec![Entry::Group(group)],
        "in the place of the tab that held it"
    );
    let tabs = state.group_tabs(group);
    assert_eq!(tabs.len(), 2, "with the inner dock's tabs in it");
    for tab in tabs {
        assert_eq!(state.home(tab), Some(DockHome::Group(group)));
    }
    assert!(
        harness.document().find_test_id("dock.grip.inner").is_none(),
        "the inner dock draws no pane of its own"
    );
    assert_eq!(
        harness.document().find_test_ids("dock.grip.joined:2/inner").len(),
        1,
        "the outer dock draws the inner dock's pane"
    );
    let panel = harness.rect(harness.find("content.10"));
    assert!(
        panel.left() > WIDE_VIEWPORT.x * 0.3,
        "the inner tab's body is placed in the outer dock's pane, at {panel:?}"
    );
    dock_tab(harness.document(), nested.node, "Nested");
    dock_tab(harness.document(), nested.node, "Inner 11");
}

use super::*;
use crate::dock::unstyled::{DockApi, Entry, TabId};

#[test]
fn dropping_a_dock_tab_on_the_middle_of_a_pane_groups_it_with_the_tab_shown() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = dock_of(3);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.click(harness.center(dock_tab(harness.document(), dock, "Tab 2")));
    harness.frame(Vec::new());
    let dragged = harness.center(dock_tab(harness.document(), dock, "Tab 3"));
    let onto = harness.rect(harness.find("content.2")).center();

    harness.drag(dragged, onto);
    harness.frame(Vec::new());

    let state = harness.document().api::<DockApi>(dock).state();
    let leaf = state.leaves(state.main())[0];
    let entries = state.entries(leaf);
    let Some(Entry::Group(group)) = entries.get(1).copied() else {
        panic!("the tab the pane showed became a group, not {entries:?}");
    };
    assert_eq!(entries.len(), 2, "the two tabs share one place in the bar");
    assert_eq!(
        state.group_tabs(group),
        vec![TabId::new(2), TabId::new(3)],
        "the group holds the tab the pane showed and the tab dropped"
    );
    let title = dock_tab(harness.document(), dock, "Tab 2, Tab 3");
    let inner = dock_tab(harness.document(), dock, "Tab 3");
    assert!(
        harness.rect(inner).top() > harness.rect(title).bottom(),
        "the tabs of the group sit in a second bar below the tab of the group"
    );
    let panel = harness.rect(harness.find("content.3"));
    assert!(
        panel.top() > harness.rect(inner).bottom(),
        "the tab shown inside the group lays its panel out below the second bar"
    );
}

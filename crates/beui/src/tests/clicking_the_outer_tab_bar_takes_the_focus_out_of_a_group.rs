use super::*;
use crate::dock::unstyled::{DockApi, Entry, TabId};

#[test]
fn clicking_the_outer_tab_bar_takes_the_focus_out_of_a_group() {
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
    let outer = state.leaves(state.main())[0];
    assert_ne!(
        state.focused_leaf(),
        Some(outer),
        "the focus starts in the group, on the tab dropped"
    );

    let group = harness.rect(dock_tab(harness.document(), dock, "Tab 2, Tab 3"));
    harness.click(pos2(group.right() + 60.0, group.center().y));
    harness.frame(Vec::new());
    assert_eq!(
        harness
            .document()
            .api::<DockApi>(dock)
            .state()
            .focused_leaf(),
        Some(outer),
        "a press on the outer tab bar focuses the outer pane"
    );

    harness.key(Key::Tab, Modifiers::CTRL);
    harness.frame(Vec::new());

    let state = harness.document().api::<DockApi>(dock).state();
    assert_eq!(
        state.active_entry(outer),
        Some(Entry::Tab(TabId::new(1))),
        "Ctrl+Tab walks the outer tab bar, from the group back round to the first tab"
    );
}

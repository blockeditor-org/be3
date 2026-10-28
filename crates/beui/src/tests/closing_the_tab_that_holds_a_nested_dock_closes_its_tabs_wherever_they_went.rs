use super::*;
use crate::dock::unstyled::TabId;

#[test]
fn closing_the_tab_that_holds_a_nested_dock_closes_its_tabs_wherever_they_went() {
    let _root = crate::reactive::RootScope::new();
    let (document, nested) = nested_dock();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    pick_dock_tab_menu(&mut harness, nested.node, "Inner 10", "Unlock");
    let from = harness.center(dock_tab(harness.document(), nested.node, "Inner 10"));
    harness.drag(from, pos2(WIDE_VIEWPORT.x * 0.15, WIDE_VIEWPORT.y / 2.0));
    harness.frame(Vec::new());
    assert_eq!(
        nested.outer.state().get_untracked().all_tabs().len(),
        3,
        "tab 10 moved out beside the side tab, tab 11 stayed in the group"
    );

    pick_dock_tab_menu(&mut harness, nested.node, "Nested", "Close group");
    harness.frame(Vec::new());
    let state = nested.outer.state().get_untracked();
    assert_eq!(
        nested.hosts.get_untracked(),
        Vec::<u64>::new(),
        "closing the group closes the hosting tab"
    );
    assert_eq!(
        state.all_tabs(),
        vec![TabId::new(1)],
        "and every tab of the inner dock, the one moved out too"
    );
    assert!(state.find_group("joined:2").is_none());
}

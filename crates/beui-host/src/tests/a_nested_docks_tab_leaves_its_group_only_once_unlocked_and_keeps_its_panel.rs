use super::*;

#[test]
fn a_nested_docks_tab_leaves_its_group_only_once_unlocked_and_keeps_its_panel() {
    let _root = crate::reactive::RootScope::new();
    let (document, nested) = nested_dock();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let state = nested.outer.state().get_untracked();
    let group = state.find_group("joined:2").expect("the inner dock joined");
    let tab = state.group_tabs(group)[0];
    let side = state.leaves(state.main())[0];
    let panel = harness.find("content.10");
    let into_side = pos2(WIDE_VIEWPORT.x * 0.15, WIDE_VIEWPORT.y / 2.0);

    let from = harness.center(dock_tab(harness.document(), nested.node, "Inner 10"));
    harness.drag(from, into_side);
    harness.frame(Vec::new());
    let state = nested.outer.state().get_untracked();
    assert!(
        state.group_tabs(group).contains(&tab),
        "a locked inner tab stays in its group"
    );

    pick_dock_tab_menu(&mut harness, nested.node, "Inner 10", "Unlock");
    let from = harness.center(dock_tab(harness.document(), nested.node, "Inner 10"));
    harness.drag(from, into_side);
    harness.frame(Vec::new());
    let state = nested.outer.state().get_untracked();
    assert_eq!(
        state.find(tab).map(|position| position.leaf),
        Some(side),
        "unlocked, it goes to another pane of the outer dock"
    );
    assert_eq!(
        harness.find("content.10"),
        panel,
        "and keeps the panel the inner dock built for it"
    );
    assert!(
        harness.rect(panel).right() < WIDE_VIEWPORT.x * 0.35,
        "laid out in the pane it went to"
    );
    assert!(
        nested.inner.contains(&10),
        "the inner dock still holds the tab"
    );
}

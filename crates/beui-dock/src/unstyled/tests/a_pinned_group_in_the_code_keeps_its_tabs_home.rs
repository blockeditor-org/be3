use super::*;

#[test]
fn a_pinned_group_in_the_code_keeps_its_tabs_home() {
    let mut state = DockState::default();
    let tools = DockSpecEntry::Group {
        key: "tools".to_owned(),
        pinned: true,
        root: Box::new(pane("tools", &[2, 3])),
    };
    let main = DockSpecNode::Pane(DockSpecPane {
        entries: vec![DockSpecEntry::Tab(TabId::new(1)), tools],
        ..pane_spec("main")
    });
    state.reconcile(&spec(main));

    let leaf = state.leaves(state.main())[0];
    let entries = state.entries(leaf);
    assert_eq!(entries.len(), 2, "the group sits beside the tab in the bar");
    let Entry::Group(group) = entries[1] else {
        panic!("the second entry is the group");
    };
    assert!(state.is_pinned(group), "the group is pinned as declared");
    assert!(
        state.is_tab_pinned(TabId::new(2)),
        "the tabs of a pinned group are pinned to it"
    );
    assert_eq!(state.group_key(group), Some("tools"));
}

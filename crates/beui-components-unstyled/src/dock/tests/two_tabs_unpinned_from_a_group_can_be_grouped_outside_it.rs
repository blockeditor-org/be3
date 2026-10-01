use super::*;

#[test]
fn two_tabs_unpinned_from_a_group_can_be_grouped_outside_it() {
    let mut state = DockState::new([TabId::new(1)]);
    let leaf = state.leaves(state.main())[0];
    let layout = DockTree::Tabs {
        entries: vec![
            DockTreeEntry::Tab(TabId::new(10)),
            DockTreeEntry::Tab(TabId::new(11)),
            DockTreeEntry::Tab(TabId::new(12)),
        ],
        active: 0,
        vertical: false,
        sidebar: SIDEBAR_WIDTH,
    };
    let group = state.insert_pinned_group(leaf, 1, &layout);
    for tab in [10, 11] {
        state.set_tab_pinned(TabId::new(tab), false);
        state.drop_tab(TabId::new(tab), DockDrop::Tab { leaf, index: 0 });
    }
    assert_eq!(state.group_tabs(group), vec![TabId::new(12)]);
    let onto = state
        .entries(leaf)
        .iter()
        .position(|entry| *entry == Entry::Tab(TabId::new(11)))
        .expect("the unpinned tab sits outside the group");
    let target = DockDrop::Group { leaf, index: onto };

    assert!(state.admits(Entry::Tab(TabId::new(10)), target));
    state.drop_tab(TabId::new(10), target);

    let Some(Entry::Group(made)) = state
        .entries(leaf)
        .iter()
        .copied()
        .find(|entry| matches!(entry, Entry::Group(made) if *made != group))
    else {
        panic!("the two tabs make a group, not {:?}", state.entries(leaf));
    };
    assert_eq!(state.group_tabs(made), vec![TabId::new(11), TabId::new(10)]);
    assert_eq!(state.group_tabs(group), vec![TabId::new(12)]);
}

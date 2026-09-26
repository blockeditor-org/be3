use super::*;

#[test]
fn a_pinned_tab_stays_in_its_group_until_it_is_unpinned() {
    let mut state = DockState::new([TabId::new(1)]);
    let leaf = state.leaves(state.main())[0];
    let layout = DockTree::Tabs {
        entries: vec![
            DockTreeEntry::Tab(TabId::new(10)),
            DockTreeEntry::Tab(TabId::new(11)),
        ],
        active: 0,
        vertical: false,
        sidebar: SIDEBAR_WIDTH,
    };
    let group = state.insert_pinned_group(leaf, 1, &layout);
    let window = DockDrop::Window {
        pos: Pos2::new(40.0, 40.0),
    };
    let outside = DockDrop::Tab { leaf, index: 0 };

    assert!(state.is_tab_pinned(TabId::new(10)));
    assert!(!state.admits(Entry::Tab(TabId::new(10)), window));
    assert!(!state.admits(Entry::Tab(TabId::new(10)), outside));
    let inner = state.tree_leaves(Tree::Group(group))[0];
    assert!(
        !state.admits_leaf(inner, window),
        "the pane holding pinned tabs cannot be dragged out either"
    );
    state.drop_tab(TabId::new(10), window);
    assert_eq!(state.group_tabs(group), vec![TabId::new(10), TabId::new(11)]);

    state.set_tab_pinned(TabId::new(10), false);
    state.drop_tab(TabId::new(10), window);

    assert_eq!(state.group_tabs(group), vec![TabId::new(11)]);
    state.set_tab_pinned(TabId::new(10), true);
    assert!(
        !state.is_tab_pinned(TabId::new(10)),
        "a tab away from its group cannot be pinned"
    );

    let inner = state.tree_leaves(Tree::Group(group))[0];
    state.drop_tab(TabId::new(10), DockDrop::Pane { leaf: inner });
    state.set_tab_pinned(TabId::new(10), true);

    assert!(state.is_tab_pinned(TabId::new(10)));
    assert_eq!(state.group_tabs(group), vec![TabId::new(11), TabId::new(10)]);
}

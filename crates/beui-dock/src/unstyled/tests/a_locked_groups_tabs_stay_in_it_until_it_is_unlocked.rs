use super::*;

#[test]
fn a_locked_groups_tabs_stay_in_it_until_it_is_unlocked() {
    let mut state = DockState::new([TabId::new(1)]);
    let leaf = state.leaves(state.main())[0];
    let layout = DockTree::Tabs {
        entries: vec![
            DockTreeEntry::Tab(TabId::new(10)),
            DockTreeEntry::Tab(TabId::new(11)),
        ],
        active: 0,
        vertical: false,
        icons_only: false,
        sidebar: SIDEBAR_WIDTH,
        bare: false,
    };
    let group = state.insert_pinned_group(leaf, 1, &layout);
    let place = DockHome::Group(group);
    let window = DockDrop::Window {
        pos: Pos2::new(40.0, 40.0),
    };
    let outside = DockDrop::Tab { leaf, index: 0 };

    assert!(state.is_locked(&place));
    assert!(state.is_tab_locked(TabId::new(10)));
    assert!(!state.admits(Entry::Tab(TabId::new(10)), window));
    assert!(!state.admits(Entry::Tab(TabId::new(10)), outside));
    let inner = state.tree_leaves(Tree::Group(group))[0];
    assert!(
        !state.admits_leaf(inner, window),
        "the pane holding locked tabs cannot be dragged out either"
    );
    state.drop_tab(TabId::new(10), window);
    assert_eq!(
        state.group_tabs(group),
        vec![TabId::new(10), TabId::new(11)]
    );

    state.set_locked(place.clone(), false);
    assert!(
        !state.is_tab_locked(TabId::new(11)),
        "unlocking frees every tab"
    );
    state.drop_tab(TabId::new(10), window);
    assert_eq!(state.group_tabs(group), vec![TabId::new(11)]);

    state.set_locked(place, true);
    assert!(
        state.is_tab_locked(TabId::new(11)),
        "locking holds the tabs at home"
    );
    assert!(
        !state.is_tab_locked(TabId::new(10)),
        "and leaves the tab that is away free"
    );
    assert!(state.admits(Entry::Tab(TabId::new(10)), outside));

    let inner = state.tree_leaves(Tree::Group(group))[0];
    state.drop_tab(TabId::new(10), DockDrop::Pane { leaf: inner });
    assert!(
        state.is_tab_locked(TabId::new(10)),
        "back home, it is locked again"
    );
    assert_eq!(
        state.group_tabs(group),
        vec![TabId::new(11), TabId::new(10)]
    );
}

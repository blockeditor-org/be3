use super::*;

#[test]
fn a_pinned_group_only_takes_the_tabs_that_live_in_it() {
    let mut state = DockState::new([TabId::new(1)]);
    let leaf = state.leaves(state.main())[0];
    let layout = DockTree::Tabs {
        entries: vec![DockTreeEntry::Tab(TabId::new(10))],
        active: 0,
    };
    let group = state.insert_pinned_group(leaf, 1, &layout);
    let inner = state.tree_leaves(Tree::Group(group))[0];

    state.drop_tab(TabId::new(1), DockDrop::Pane { leaf: inner });

    assert_eq!(
        state.group_tabs(group),
        vec![TabId::new(10)],
        "a tab from outside is refused"
    );

    state.drop_tab(
        TabId::new(10),
        DockDrop::Window {
            pos: Pos2::new(40.0, 40.0),
        },
    );

    assert!(state.group_tabs(group).is_empty(), "its own tab can leave");
    assert_eq!(state.windows().len(), 1);

    let inner = state.tree_leaves(Tree::Group(group))[0];
    state.drop_tab(TabId::new(10), DockDrop::Pane { leaf: inner });

    assert_eq!(
        state.group_tabs(group),
        vec![TabId::new(10)],
        "and its own tab can come back"
    );
    assert!(state.windows().is_empty());
}

use super::*;

fn tabs(ids: &[u64]) -> DockTree {
    DockTree::Tabs {
        entries: ids
            .iter()
            .map(|id| DockTreeEntry::Tab(TabId::new(*id)))
            .collect(),
        active: 0,
    }
}

#[test]
fn a_pinned_group_keeps_its_last_tab_and_survives_being_emptied() {
    let mut state = DockState::new([TabId::new(1)]);
    let leaf = state.leaves(state.main())[0];
    let group = state.insert_pinned_group(leaf, 1, &tabs(&[10, 11]));

    state.remove(TabId::new(11));

    assert_eq!(
        state.entries(leaf),
        vec![Entry::Tab(TabId::new(1)), Entry::Group(group)],
        "a pinned group with one tab stays a group"
    );

    state.remove(TabId::new(10));

    assert!(state.is_pinned(group), "an emptied pinned group is kept");
    assert_eq!(state.tree(Tree::Group(group)), Some(tabs(&[])));

    state.remove(TabId::new(1));

    assert_eq!(
        state.entries(leaf),
        vec![Entry::Group(group)],
        "a pane holding only a pinned group keeps the group"
    );
}

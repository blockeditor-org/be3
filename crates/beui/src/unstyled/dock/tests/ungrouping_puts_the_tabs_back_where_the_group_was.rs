use super::*;

#[test]
fn ungrouping_puts_the_tabs_back_where_the_group_was() {
    let mut state = DockState::new((1..=4).map(TabId::new));
    let leaf = state.leaves(state.main())[0];
    state.split_with_next(leaf, 1);
    let Some(Entry::Group(group)) = state.entries(leaf).get(1).copied() else {
        panic!("the tabs were grouped");
    };
    state.show(TabId::new(2));

    state.ungroup(group);

    assert_eq!(
        state.entries(leaf),
        [1, 2, 3, 4].map(|tab| Entry::Tab(TabId::new(tab))),
        "the tabs of the group are laid back into the bar in its place"
    );
    assert_eq!(
        state.active_tab(leaf),
        Some(TabId::new(2)),
        "the tab that had the focus inside the group is the one shown"
    );
    assert!(
        state.tree_leaves(Tree::Group(group)).is_empty(),
        "the group itself is gone"
    );
}

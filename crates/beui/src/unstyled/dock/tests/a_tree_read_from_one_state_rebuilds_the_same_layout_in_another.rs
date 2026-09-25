use super::*;

#[test]
fn a_tree_read_from_one_state_rebuilds_the_same_layout_in_another() {
    let mut state = DockState::new((1..=4).map(TabId::new));
    let leaf = state.leaves(state.main())[0];
    state.split(leaf, Side::Right, 0.3, vec![TabId::new(5)]);
    state.drop_tab(TabId::new(3), DockDrop::Group { leaf, index: 1 });
    let tree = state.tree(Tree::Surface(state.main())).expect("the main surface has a tree");

    let rebuilt = DockState::from_tree(&tree);

    assert_eq!(rebuilt.tree(Tree::Surface(rebuilt.main())), Some(tree.clone()));
    assert_eq!(rebuilt.all_tabs(), state.all_tabs());
    let DockTree::Split { first, .. } = &tree else {
        panic!("the split survives, not {tree:?}");
    };
    let DockTree::Tabs { entries, .. } = first.as_ref() else {
        panic!("the left side is a tab bar, not {first:?}");
    };
    assert!(
        matches!(entries.get(1), Some(DockTreeEntry::Group(_))),
        "the group survives"
    );
}

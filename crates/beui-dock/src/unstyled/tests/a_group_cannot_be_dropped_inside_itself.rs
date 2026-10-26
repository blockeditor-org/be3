use super::*;

#[test]
fn a_group_cannot_be_dropped_inside_itself() {
    let mut state = DockState::new((1..=3).map(TabId::new));
    let leaf = state.leaves(state.main())[0];
    state.group_with_next(leaf, 1);
    let Some(Entry::Group(group)) = state.entries(leaf).get(1).copied() else {
        panic!("the tabs were grouped");
    };
    let inner = state.tree_leaves(Tree::Group(group))[0];
    let before = state.clone();

    state.drop_entry(Entry::Group(group), DockDrop::Pane { leaf: inner });
    state.drop_entry(
        Entry::Group(group),
        DockDrop::Split {
            leaf: inner,
            side: Side::Left,
        },
    );

    assert_eq!(
        state, before,
        "dropping a group into a pane of its own changes nothing"
    );
}

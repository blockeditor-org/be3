use super::*;

#[test]
fn splitting_a_tab_with_the_next_makes_a_group_holding_a_split_view() {
    let mut state = DockState::new((1..=4).map(TabId::new));
    let leaf = state.leaves(state.main())[0];

    state.split_with_next(leaf, 1);

    let Some(Entry::Group(group)) = state.entries(leaf).get(1).copied() else {
        panic!("the tab split is replaced by a group");
    };
    assert_eq!(
        state.entries(leaf).len(),
        3,
        "the two tabs become one entry in the tab bar"
    );
    let inner = state.tree_leaves(Tree::Group(group));
    assert_eq!(
        inner
            .iter()
            .map(|leaf| state.entries(*leaf))
            .collect::<Vec<_>>(),
        vec![
            vec![Entry::Tab(TabId::new(2))],
            vec![Entry::Tab(TabId::new(3))]
        ],
        "each tab has a pane of its own inside the group"
    );
    let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(406.0, 200.0));
    let layout = layout_tree(&state, Tree::Group(group), area, 6.0);
    assert_eq!(
        layout.splitters.len(),
        1,
        "the group lays its panes out side by side with a bar between"
    );
    assert!(
        state.is_nested(inner[0]) && !state.is_nested(leaf),
        "the panes inside the group are nested and the outer one is not"
    );
}

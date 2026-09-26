use super::*;

#[test]
fn splitting_off_a_pane_keeps_its_tabs_and_its_sidebar() {
    let mut state = DockState::new([TabId::new(1)]);
    let left = state.leaves(state.main())[0];
    let right = state
        .split(left, Side::Right, 0.5, vec![TabId::new(2), TabId::new(3)])
        .expect("the pane was split");
    state.set_active_index(right, 1);
    state.set_vertical(right, true);

    state.drop_leaf(
        right,
        DockDrop::Split {
            leaf: left,
            side: Side::Below,
        },
    );

    let leaves = state.leaves(state.main());
    assert_eq!(leaves.len(), 2, "the pane moved rather than being copied");
    let moved = leaves[1];
    assert_eq!(
        state.entries(moved),
        vec![Entry::Tab(TabId::new(2)), Entry::Tab(TabId::new(3))],
        "the pane takes all of its tabs with it"
    );
    assert_eq!(
        state.active_tab(moved),
        Some(TabId::new(3)),
        "the pane goes on showing the tab it showed"
    );
    assert!(
        state.is_vertical(moved),
        "the pane keeps its tabs in a sidebar"
    );

    state.drop_leaf(moved, DockDrop::Pane { leaf: moved });
    assert_eq!(
        state.entries(moved).len(),
        2,
        "dropping a pane on itself leaves it alone"
    );
}

use super::*;

#[test]
fn a_group_left_with_one_tab_becomes_that_tab_again() {
    let mut state = DockState::new((1..=4).map(TabId::new));
    let leaf = state.leaves(state.main())[0];
    state.group_with_next(leaf, 1);

    state.remove(TabId::new(3));

    assert_eq!(
        state.entries(leaf),
        [1, 2, 4].map(|tab| Entry::Tab(TabId::new(tab))),
        "the tab left over takes the place the group had"
    );
    assert_eq!(
        state.focused_leaf(),
        Some(leaf),
        "the focus moves out of the group that was dissolved"
    );
}

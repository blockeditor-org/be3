use super::*;

#[test]
fn a_pane_holding_only_a_group_takes_the_tabs_of_the_group() {
    let mut state = DockState::new((1..=3).map(TabId::new));
    let leaf = state.leaves(state.main())[0];
    state.group_with_next(leaf, 1);

    state.remove(TabId::new(1));

    assert_eq!(
        state.leaves(state.main()),
        vec![leaf],
        "the pane keeps its place"
    );
    assert_eq!(
        state.entries(leaf),
        [2, 3].map(|tab| Entry::Tab(TabId::new(tab))),
        "a tab bar holding one group is folded into the tabs of the group"
    );
    assert_eq!(
        state.active_tab(leaf),
        Some(TabId::new(3)),
        "the tab the group was showing is still the one shown"
    );
}

use super::*;

#[test]
fn showing_a_tab_inside_a_group_selects_the_group() {
    let mut state = DockState::new((1..=3).map(TabId::new));
    let leaf = state.leaves(state.main())[0];
    state.group_with_next(leaf, 1);
    state.set_active_index(leaf, 0);
    state.focus(leaf);

    state.show(TabId::new(2));

    assert_eq!(
        state.active_index(leaf),
        1,
        "the outer tab bar switches to the group holding the tab"
    );
    assert_eq!(
        state.focused_tab(),
        Some(TabId::new(2)),
        "inside the group the tab asked for is shown"
    );
}

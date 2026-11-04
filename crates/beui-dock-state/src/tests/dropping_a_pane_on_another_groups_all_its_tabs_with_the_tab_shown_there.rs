use super::*;

#[test]
fn dropping_a_pane_on_another_groups_all_its_tabs_with_the_tab_shown_there() {
    let mut state = DockState::new([TabId::new(1), TabId::new(4)]);
    let left = state.leaves(state.main())[0];
    let right = state
        .split(left, Side::Right, 0.5, vec![TabId::new(2), TabId::new(3)])
        .expect("the pane was split");
    state.set_active_index(right, 1);
    state.set_active_index(left, 0);

    state.drop_leaf(
        right,
        DockDrop::Group {
            leaf: left,
            index: 0,
        },
    );

    let leaves = state.leaves(state.main());
    assert_eq!(leaves, vec![left], "the dragged pane is gone");
    let Some(Entry::Group(group)) = state.entries(left).first().copied() else {
        panic!("the tab dropped on became a group");
    };
    assert_eq!(
        state.group_tabs(group),
        vec![TabId::new(1), TabId::new(2), TabId::new(3)],
        "the group holds the tab dropped on and every tab of the pane"
    );
    assert_eq!(
        state.focused_tab(),
        Some(TabId::new(3)),
        "the group shows the tab the pane showed"
    );
}

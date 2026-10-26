use super::*;

#[test]
fn a_tab_dropped_on_another_tab_bar_lands_at_the_place_it_was_dropped() {
    let mut state = DockState::new([TabId::new(1), TabId::new(2)]);
    let left = state.leaves(state.main())[0];
    let right = state
        .split(left, Side::Right, 0.5, vec![TabId::new(3), TabId::new(4)])
        .expect("the pane was split");

    state.drop_tab(
        TabId::new(1),
        DockDrop::Tab {
            leaf: right,
            index: 1,
        },
    );

    assert_eq!(
        state.entries(right),
        vec![
            Entry::Tab(TabId::new(3)),
            Entry::Tab(TabId::new(1)),
            Entry::Tab(TabId::new(4))
        ],
        "the tab lands between the tabs it was dropped between"
    );
    assert_eq!(
        state.entries(left),
        vec![Entry::Tab(TabId::new(2))],
        "the tab leaves the pane it came from"
    );
    assert_eq!(
        state.active_tab(right),
        Some(TabId::new(1)),
        "the pane it landed in shows it"
    );
    assert_eq!(
        state.focused_leaf(),
        Some(right),
        "the pane it landed in takes the focus"
    );
}

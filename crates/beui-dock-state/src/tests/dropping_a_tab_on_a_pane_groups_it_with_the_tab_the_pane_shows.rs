use super::*;

#[test]
fn dropping_a_tab_on_a_pane_groups_it_with_the_tab_the_pane_shows() {
    let mut state = DockState::new((1..=4).map(TabId::new));
    let leaf = state.leaves(state.main())[0];
    state.set_active_index(leaf, 1);
    let shown = DockDrop::Group {
        leaf,
        index: state.active_index(leaf),
    };
    assert!(
        !state.admits(Entry::Tab(TabId::new(2)), shown),
        "the tab a pane shows cannot be grouped with itself"
    );

    state.drop_tab(TabId::new(3), shown);

    let entries = state.entries(leaf);
    let Some(Entry::Group(group)) = entries.get(1).copied() else {
        panic!("the tab dropped on is replaced by a group, not {entries:?}");
    };
    assert_eq!(
        entries,
        vec![
            Entry::Tab(TabId::new(1)),
            Entry::Group(group),
            Entry::Tab(TabId::new(4))
        ],
        "the group takes the place of the tab it was dropped on"
    );
    assert_eq!(
        state.group_tabs(group),
        vec![TabId::new(2), TabId::new(3)],
        "the group holds the tab dropped on and the tab dropped"
    );
    assert_eq!(
        state.focused_tab(),
        Some(TabId::new(3)),
        "the dropped tab is the one shown"
    );
    assert_eq!(
        state.active_entry(leaf),
        Some(Entry::Group(group)),
        "the outer tab bar shows the group"
    );
    assert_eq!(
        state.surface_of(state.find(TabId::new(3)).expect("the tab is docked").leaf),
        Some(state.main()),
        "a tab inside a group still belongs to the surface the group is on"
    );
}

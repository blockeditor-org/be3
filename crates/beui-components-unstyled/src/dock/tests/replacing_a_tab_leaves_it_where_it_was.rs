use super::*;

#[test]
fn replacing_a_tab_leaves_it_where_it_was() {
    let mut state = DockState::new([TabId::new(1), TabId::new(2), TabId::new(3)]);
    let leaf = state.leaves(state.main())[0];
    state.set_active_index(leaf, 1);

    assert!(
        state.replace(TabId::new(2), TabId::new(9)),
        "a tab that is open can be replaced"
    );

    assert_eq!(
        state.entries(leaf),
        vec![
            Entry::Tab(TabId::new(1)),
            Entry::Tab(TabId::new(9)),
            Entry::Tab(TabId::new(3))
        ],
        "the replacement takes the place the tab had"
    );
    assert_eq!(
        state.active_tab(leaf),
        Some(TabId::new(9)),
        "the pane goes on showing the place it was showing"
    );
    assert!(
        !state.replace(TabId::new(2), TabId::new(8)),
        "a tab that is not open is not replaced"
    );
}

use super::*;

#[test]
fn closing_the_last_tab_of_a_window_closes_the_window() {
    let mut state = DockState::new([TabId::new(1)]);
    let window = state.open_window(
        Rect::from_min_size(Pos2::new(40.0, 40.0), FLOATING_SIZE),
        vec![TabId::new(2), TabId::new(3)],
    );

    state.remove(TabId::new(2));
    assert_eq!(
        state.windows(),
        vec![window],
        "a window with a tab left in it stays open"
    );

    state.remove(TabId::new(3));
    assert!(
        state.windows().is_empty(),
        "the window closes once its last tab is gone"
    );
    assert_eq!(
        state.focused_leaf(),
        Some(state.leaves(state.main())[0]),
        "the focus falls back to a pane that is still there"
    );
}

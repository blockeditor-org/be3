use super::*;

#[test]
fn a_locked_tab_cannot_float_into_a_window() {
    let mut state = DockState::default();
    state.reconcile(&sidebar_and_editors(&[1], &[10, 11]));
    let window = DockDrop::Window {
        pos: Pos2::new(40.0, 40.0),
    };
    let editors = leaf_of(&state, 10);

    assert!(!state.admits(Entry::Tab(TabId::new(10)), window));
    assert!(!state.admits_leaf(editors, window));
    state.drop_tab(TabId::new(10), window);
    state.drop_leaf(editors, window);

    assert!(state.windows().is_empty(), "no window opened");
    assert_eq!(leaf_of(&state, 10), editors);
}

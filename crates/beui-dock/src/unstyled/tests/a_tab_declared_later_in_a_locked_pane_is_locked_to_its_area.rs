use super::*;

#[test]
fn a_tab_declared_later_in_a_locked_pane_is_locked_to_its_area() {
    let mut state = DockState::default();
    state.reconcile(&sidebar_and_editors(&[1], &[10]));
    state.reconcile(&sidebar_and_editors(&[1], &[10, 11]));

    assert_eq!(leaf_of(&state, 11), leaf_of(&state, 10));
    assert!(state.is_tab_pinned(TabId::new(11)));
    assert_eq!(
        state.home(TabId::new(11)),
        Some(DockHome::Area("editors".to_owned()))
    );
    let files = leaf_of(&state, 1);
    assert!(!state.admits(Entry::Tab(TabId::new(11)), DockDrop::Pane { leaf: files }));
}

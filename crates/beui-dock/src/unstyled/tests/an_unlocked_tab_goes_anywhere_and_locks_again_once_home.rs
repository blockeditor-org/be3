use super::*;

#[test]
fn an_unlocked_tab_goes_anywhere_and_locks_again_once_home() {
    let mut state = DockState::default();
    state.reconcile(&sidebar_and_editors(&[1, 2], &[10]));
    let files = leaf_of(&state, 1);
    let editors = leaf_of(&state, 10);

    state.set_tab_pinned(TabId::new(1), false);
    assert!(!state.is_tab_pinned(TabId::new(1)));
    assert!(
        state.admits(Entry::Tab(TabId::new(1)), DockDrop::Pane { leaf: editors }),
        "an unlocked tab may join a locked area that is not its home"
    );
    state.drop_tab(TabId::new(1), DockDrop::Pane { leaf: editors });
    assert_eq!(leaf_of(&state, 1), editors);

    state.set_tab_pinned(TabId::new(1), true);
    assert!(
        !state.is_tab_pinned(TabId::new(1)),
        "a tab away from its area cannot be locked"
    );

    state.drop_tab(
        TabId::new(1),
        DockDrop::Window {
            pos: Pos2::new(40.0, 40.0),
        },
    );
    assert_eq!(state.windows().len(), 1, "an unlocked tab floats");

    state.drop_tab(TabId::new(1), DockDrop::Pane { leaf: files });
    assert!(state.at_home(TabId::new(1)));
    state.set_tab_pinned(TabId::new(1), true);
    assert!(state.is_tab_pinned(TabId::new(1)), "back home it locks again");
    assert!(!state.admits(Entry::Tab(TabId::new(1)), DockDrop::Pane { leaf: editors }));
}

use super::*;

#[test]
fn a_tab_of_an_unlocked_area_goes_anywhere_and_locks_again_once_home() {
    let mut state = DockState::default();
    state.reconcile(&sidebar_and_editors(&[1, 2], &[10]));
    let files = leaf_of(&state, 1);
    let editors = leaf_of(&state, 10);
    let place = DockHome::Area("files".to_owned());

    state.set_locked(place.clone(), false);
    assert!(!state.is_tab_locked(TabId::new(1)));
    assert!(
        state.admits(Entry::Tab(TabId::new(1)), DockDrop::Pane { leaf: editors }),
        "a tab of an unlocked area may join a locked area that is not its home"
    );
    state.drop_tab(TabId::new(1), DockDrop::Pane { leaf: editors });
    assert_eq!(leaf_of(&state, 1), editors);

    state.set_locked(place.clone(), true);
    assert!(state.is_tab_locked(TabId::new(2)));
    assert!(
        !state.is_tab_locked(TabId::new(1)),
        "a tab away from its area stays free once the area locks again"
    );

    state.drop_tab(
        TabId::new(1),
        DockDrop::Window {
            pos: Pos2::new(40.0, 40.0),
        },
    );
    assert_eq!(state.windows().len(), 1, "a free tab floats");

    state.drop_tab(TabId::new(1), DockDrop::Pane { leaf: files });
    assert!(state.at_home(TabId::new(1)));
    assert!(
        state.is_tab_locked(TabId::new(1)),
        "back home it is locked again"
    );
    assert!(!state.admits(Entry::Tab(TabId::new(1)), DockDrop::Pane { leaf: editors }));
}

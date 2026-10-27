use super::*;

#[test]
fn a_locked_tab_found_outside_its_area_is_moved_back_home() {
    let open = spec(split(
        "workspace",
        Direction::Horizontal,
        0.25,
        pane("files", &[1, 2]),
        pane("editors", &[10]),
    ));
    let mut state = DockState::default();
    state.reconcile(&open);
    let editors = leaf_of(&state, 10);
    state.drop_tab(TabId::new(1), DockDrop::Tab { leaf: editors, index: usize::MAX });
    state.drop_tab(
        TabId::new(2),
        DockDrop::Window {
            pos: Pos2::new(40.0, 40.0),
        },
    );
    assert_eq!(
        state.leaves(state.main()),
        vec![editors],
        "the files pane is gone"
    );

    state.reconcile(&sidebar_and_editors(&[1, 2], &[10]));

    let files = leaf_of(&state, 1);
    assert_ne!(files, editors);
    assert_eq!(state.area_of(files), Some("files"));
    assert_eq!(state.pane_key(files), Some("files"));
    assert_eq!(
        state.entries(files),
        vec![Entry::Tab(TabId::new(1)), Entry::Tab(TabId::new(2))],
        "both tabs are back in their area"
    );
    assert!(state.windows().is_empty(), "the emptied window closed");
    assert_eq!(state.entries(editors), vec![Entry::Tab(TabId::new(10))]);
}

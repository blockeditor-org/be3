use super::*;

#[test]
fn a_locked_tab_dropped_outside_its_area_stays_put() {
    let mut state = DockState::default();
    state.reconcile(&sidebar_and_editors(&[1, 2], &[10, 11]));
    let files = leaf_of(&state, 1);
    let editors = leaf_of(&state, 10);
    assert_eq!(state.area_of(files), Some("files"));
    assert_eq!(state.area_of(editors), Some("editors"));
    assert!(state.is_tab_locked(TabId::new(1)));
    assert_eq!(
        state.home(TabId::new(1)),
        Some(DockHome::Area("files".to_owned()))
    );

    let outside = [
        DockDrop::Pane { leaf: editors },
        DockDrop::Tab {
            leaf: editors,
            index: 0,
        },
        DockDrop::Group {
            leaf: editors,
            index: 0,
        },
        DockDrop::Split {
            leaf: editors,
            side: Side::Right,
        },
    ];
    for target in outside {
        assert!(
            !state.admits(Entry::Tab(TabId::new(1)), target),
            "{target:?} lies outside the tab's area"
        );
        state.drop_tab(TabId::new(1), target);
    }
    assert!(!state.admits_leaf(files, DockDrop::Pane { leaf: editors }));
    state.drop_leaf(files, DockDrop::Pane { leaf: editors });
    state.drop_tab(TabId::new(10), DockDrop::Pane { leaf: files });

    assert_eq!(
        state.entries(files),
        vec![Entry::Tab(TabId::new(1)), Entry::Tab(TabId::new(2))]
    );
    assert_eq!(
        state.entries(editors),
        vec![Entry::Tab(TabId::new(10)), Entry::Tab(TabId::new(11))]
    );

    state.drop_tab(
        TabId::new(2),
        DockDrop::Tab {
            leaf: files,
            index: 0,
        },
    );
    assert_eq!(
        state.entries(files),
        vec![Entry::Tab(TabId::new(2)), Entry::Tab(TabId::new(1))],
        "inside its area the tab moves freely"
    );
}

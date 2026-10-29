use super::*;

#[test]
fn a_locked_area_refuses_a_tab_that_lives_nowhere() {
    let mut state = DockState::default();
    state.reconcile(&spec(split(
        "shell",
        Direction::Horizontal,
        0.25,
        locked("catalog", &[1]),
        pane("pages", &[10, 11]),
    )));
    let catalog = leaf_of(&state, 1);
    let pages = leaf_of(&state, 10);
    assert_eq!(state.home(TabId::new(10)), None);

    assert!(!state.admits(Entry::Tab(TabId::new(10)), DockDrop::Pane { leaf: catalog }));
    assert!(!state.admits(
        Entry::Tab(TabId::new(10)),
        DockDrop::Split {
            leaf: catalog,
            side: Side::Below,
        }
    ));
    assert!(!state.admits_leaf(pages, DockDrop::Pane { leaf: catalog }));

    let place = DockHome::Area("catalog".to_owned());
    state.set_locked(place.clone(), false);
    state.drop_tab(
        TabId::new(10),
        DockDrop::Split {
            leaf: catalog,
            side: Side::Below,
        },
    );
    let split_off = leaf_of(&state, 10);
    assert_ne!(split_off, catalog);
    assert_eq!(
        state.area_of(split_off),
        None,
        "a pane split off an area holding none of its tabs is not part of it"
    );

    state.drop_tab(TabId::new(11), DockDrop::Pane { leaf: catalog });
    assert_eq!(
        leaf_of(&state, 11),
        catalog,
        "an unlocked area takes any tab"
    );
    state.set_locked(place, true);
    assert!(
        state.admits(
            Entry::Tab(TabId::new(11)),
            DockDrop::Tab {
                leaf: catalog,
                index: 0
            }
        ),
        "a tab already inside may move about in it"
    );
    assert!(!state.admits(Entry::Tab(TabId::new(10)), DockDrop::Pane { leaf: catalog }));
}

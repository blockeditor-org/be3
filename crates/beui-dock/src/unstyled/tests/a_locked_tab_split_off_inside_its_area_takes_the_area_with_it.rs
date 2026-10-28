use super::*;

#[test]
fn a_locked_tab_split_off_inside_its_area_takes_the_area_with_it() {
    let mut state = DockState::default();
    state.reconcile(&sidebar_and_editors(&[1], &[10, 11, 12]));
    let editors = leaf_of(&state, 10);

    state.drop_tab(
        TabId::new(11),
        DockDrop::Split {
            leaf: editors,
            side: Side::Right,
        },
    );
    let split = leaf_of(&state, 11);
    assert_ne!(split, editors, "the tab split off into a pane of its own");
    assert_eq!(
        state.area_of(split),
        Some("editors"),
        "a pane split off a pane of the area is part of it"
    );

    state.drop_tab(
        TabId::new(12),
        DockDrop::Tab {
            leaf: split,
            index: usize::MAX,
        },
    );
    assert_eq!(
        leaf_of(&state, 12),
        split,
        "tabs of the area join any pane of it"
    );
    state.drop_tab(
        TabId::new(10),
        DockDrop::Group {
            leaf: split,
            index: 0,
        },
    );
    assert!(
        state.is_nested(leaf_of(&state, 10)),
        "tabs of the area group with each other"
    );
    assert!(
        state.at_home(TabId::new(10)),
        "a group in the area is in it"
    );
    assert!(!state.admits(
        Entry::Tab(TabId::new(1)),
        DockDrop::Tab {
            leaf: split,
            index: usize::MAX
        }
    ));

    state.drop_tab(
        TabId::new(10),
        DockDrop::Tab {
            leaf: split,
            index: usize::MAX,
        },
    );
    state.drop_tab(
        TabId::new(11),
        DockDrop::Tab {
            leaf: editors,
            index: usize::MAX,
        },
    );
    state.drop_tab(
        TabId::new(12),
        DockDrop::Tab {
            leaf: editors,
            index: usize::MAX,
        },
    );
    state.drop_tab(
        TabId::new(10),
        DockDrop::Tab {
            leaf: editors,
            index: usize::MAX,
        },
    );
    assert_eq!(
        state.leaves(state.main()).len(),
        2,
        "the area shrinks back once its split empties"
    );
}

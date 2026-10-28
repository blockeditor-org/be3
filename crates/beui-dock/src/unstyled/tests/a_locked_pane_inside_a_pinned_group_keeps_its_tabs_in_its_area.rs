use super::*;

#[test]
fn a_locked_pane_inside_a_pinned_group_keeps_its_tabs_in_its_area() {
    let mut state = DockState::default();
    let inner = split(
        "inner",
        Direction::Horizontal,
        0.5,
        locked("locked", &[10]),
        pane("free", &[11]),
    );
    let main = DockSpecNode::Pane(DockSpecPane {
        entries: vec![
            DockSpecEntry::Tab(TabId::new(1)),
            DockSpecEntry::Group {
                key: "nested".to_owned(),
                pinned: true,
                root: Box::new(inner),
            },
        ],
        ..pane_spec("main")
    });
    state.reconcile(&spec(main));
    let group = state.find_group("nested").expect("the group is open");

    assert_eq!(
        state.home(TabId::new(10)),
        Some(DockHome::Area("locked".to_owned())),
        "a tab declared in a locked pane is locked to that area, inside the group too"
    );
    assert_eq!(state.home(TabId::new(11)), Some(DockHome::Group(group)));

    let free = leaf_of(&state, 11);
    state.drop_tab(TabId::new(10), DockDrop::Pane { leaf: free });
    assert_ne!(
        leaf_of(&state, 10),
        free,
        "the locked tab stays in its area"
    );

    let main = leaf_of(&state, 1);
    state.set_tab_pinned(TabId::new(10), false);
    state.drop_tab(TabId::new(10), DockDrop::Pane { leaf: main });
    assert_eq!(leaf_of(&state, 10), main, "unlocked, it leaves the group");

    let free = leaf_of(&state, 11);
    state.drop_tab(TabId::new(10), DockDrop::Pane { leaf: free });
    assert_eq!(
        leaf_of(&state, 10),
        free,
        "and the group takes it back, as a tab whose area is inside it"
    );
}

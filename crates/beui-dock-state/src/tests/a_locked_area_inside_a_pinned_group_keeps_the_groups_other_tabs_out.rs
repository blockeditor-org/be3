use super::*;

#[test]
fn a_locked_area_inside_a_pinned_group_keeps_the_groups_other_tabs_out() {
    let mut state = DockState::default();
    let inner = split(
        "inner",
        Direction::Horizontal,
        0.3,
        locked("outline", &[10]),
        pane("views", &[11, 12]),
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
    let outline = leaf_of(&state, 10);
    let views = leaf_of(&state, 11);

    let into_outline = [
        DockDrop::Pane { leaf: outline },
        DockDrop::Tab {
            leaf: outline,
            index: 0,
        },
        DockDrop::Group {
            leaf: outline,
            index: 0,
        },
        DockDrop::Split {
            leaf: outline,
            side: Side::Left,
        },
    ];
    for target in into_outline {
        assert!(
            !state.admits(Entry::Tab(TabId::new(11)), target),
            "{target:?} is inside a locked area the tab does not live in"
        );
        state.drop_tab(TabId::new(11), target);
    }
    assert!(!state.admits_leaf(views, DockDrop::Pane { leaf: outline }));
    assert_eq!(state.entries(outline), vec![Entry::Tab(TabId::new(10))]);
    assert_eq!(state.tree_leaves(Tree::Group(group)).len(), 2);
    assert!(
        state.admits(
            Entry::Tab(TabId::new(12)),
            DockDrop::Tab {
                leaf: views,
                index: 0
            }
        ),
        "the group's own panes still take its tabs"
    );

    state.set_locked(DockHome::Group(group), false);
    assert!(
        state.admits(Entry::Tab(TabId::new(11)), DockDrop::Pane { leaf: outline }),
        "once their group is unlocked, its tabs may go into the area"
    );
    state.drop_tab(TabId::new(11), DockDrop::Pane { leaf: outline });
    assert_eq!(leaf_of(&state, 11), outline);
    assert!(
        !state.at_home(TabId::new(11)),
        "a tab in another area of its group is not at home"
    );
    state.set_locked(DockHome::Group(group), true);
    assert!(
        !state.is_tab_locked(TabId::new(11)),
        "so locking the group again leaves it free"
    );
}

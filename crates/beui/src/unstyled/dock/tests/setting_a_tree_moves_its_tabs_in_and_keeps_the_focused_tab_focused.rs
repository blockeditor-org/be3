use super::*;

#[test]
fn setting_a_tree_moves_its_tabs_in_and_keeps_the_focused_tab_focused() {
    let mut state = DockState::new([TabId::new(1)]);
    let leaf = state.leaves(state.main())[0];
    let layout = DockTree::Tabs {
        entries: vec![
            DockTreeEntry::Tab(TabId::new(10)),
            DockTreeEntry::Tab(TabId::new(11)),
        ],
        active: 1,
    };
    let group = state.insert_pinned_group(leaf, 1, &layout);
    state.show(TabId::new(11));
    state.open_window(
        Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(300.0, 200.0)),
        vec![TabId::new(12)],
    );
    state.show(TabId::new(11));

    let split = DockTree::Split {
        direction: Direction::Horizontal,
        fraction: 0.4,
        first: Box::new(DockTree::Tabs {
            entries: vec![DockTreeEntry::Tab(TabId::new(10))],
            active: 0,
        }),
        second: Box::new(DockTree::Tabs {
            entries: vec![
                DockTreeEntry::Tab(TabId::new(11)),
                DockTreeEntry::Tab(TabId::new(12)),
            ],
            active: 0,
        }),
    };
    state.set_tree(Tree::Group(group), &split);

    assert_eq!(state.tree(Tree::Group(group)), Some(split));
    assert!(
        state.windows().is_empty(),
        "a tab set into the tree leaves its window"
    );
    assert_eq!(state.home(TabId::new(12)), Some(group));
    assert_eq!(state.focused_tab(), Some(TabId::new(11)));
}

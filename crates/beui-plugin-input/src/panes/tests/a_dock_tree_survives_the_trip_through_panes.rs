use super::*;

#[test]
fn a_dock_tree_survives_the_trip_through_panes() {
    let tree = DockTree::Split {
        direction: Direction::Vertical,
        fraction: 0.3,
        first: Box::new(DockTree::Tabs {
            entries: vec![DockTreeEntry::Tab(TabId::new(1))],
            active: 0,
            vertical: false,
            sidebar: SIDEBAR_WIDTH,
        }),
        second: Box::new(DockTree::Tabs {
            entries: vec![
                DockTreeEntry::Tab(TabId::new(2)),
                DockTreeEntry::Group(DockTree::Tabs {
                    entries: vec![
                        DockTreeEntry::Tab(TabId::new(3)),
                        DockTreeEntry::Tab(TabId::new(4)),
                    ],
                    active: 1,
                    vertical: false,
                    sidebar: SIDEBAR_WIDTH,
                }),
            ],
            active: 1,
            vertical: false,
            sidebar: SIDEBAR_WIDTH,
        }),
    };

    assert_eq!(dock_tree(&pane_tree(&tree)), Some(tree));
}

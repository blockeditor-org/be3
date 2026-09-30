use super::*;

#[test]
fn an_empty_pane_in_the_plugins_layout_stays_and_is_not_rebuilt() {
    let mut state = DockState::new([WORKSPACE]);
    let docked = Docked::default();
    let mut split = layout(&[1], 0);
    split.tree = PaneTree {
        items: vec![
            PaneItem::Split {
                horizontal: true,
                fraction: 0.3,
            },
            PaneItem::Tabs {
                count: 1,
                active: 0,
                vertical: false,
                sidebar: 180.0,
            },
            PaneItem::Pane(PaneId(1)),
            PaneItem::Tabs {
                count: 0,
                active: 0,
                vertical: false,
                sidebar: 180.0,
            },
        ],
    };

    apply(&mut state, &docked, Some(&split));

    let group = docked.group.get().expect("the workspace became a group");
    assert_eq!(
        state.tree(Tree::Group(group)),
        dock_tree_with(&split.tree, &pane_tab),
        "the empty pane beside the files keeps its room"
    );

    let before = state.clone();
    apply(&mut state, &docked, Some(&split));

    assert!(
        state == before,
        "the same layout coming back leaves the dock as it was, so a divider being dragged keeps its split"
    );
}

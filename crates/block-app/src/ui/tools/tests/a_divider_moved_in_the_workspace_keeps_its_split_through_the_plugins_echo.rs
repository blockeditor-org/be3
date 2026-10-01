use super::*;
use beui::unstyled::layout_tree;
use beui_plugin_input::panes::{dock_tree, pane_tree};

#[test]
fn a_divider_moved_in_the_workspace_keeps_its_split_through_the_plugins_echo() {
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
    let area = Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 600.0));
    let splitter = layout_tree(&state, Tree::Group(group), area, 4.0).splitters[0].id;

    state.set_split_fraction(splitter, 0.4);
    let sent = arranged(&state, group);
    docked.arrangement.set(docked.arrangement.get() + 1);
    let mut plugin = DockState::from_tree(&dock_tree(&sent.tree).expect("a whole tree"));
    if let Some(focused) = sent.focused {
        plugin.show(beui_plugin_input::panes::tab_of(focused));
    }
    let mut echo = split.clone();
    echo.tree = pane_tree(
        &plugin
            .tree(Tree::Surface(plugin.main()))
            .unwrap_or_default(),
    );
    echo.arrangement = docked.arrangement.get();
    apply(&mut state, &docked, Some(&echo));

    let after = layout_tree(&state, Tree::Group(group), area, 4.0);
    assert_eq!(
        after.splitters.first().map(|splitter| splitter.id),
        Some(splitter),
        "the split being dragged is still the same split"
    );
}

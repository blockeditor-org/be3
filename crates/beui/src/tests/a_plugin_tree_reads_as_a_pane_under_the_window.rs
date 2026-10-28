use accesskit::{Node, NodeId, Rect as AccessRect, Role, Toggled, Tree, TreeId, TreeUpdate};

use super::*;
use beui_core::accessibility::{Embedded, tree_update};
use beui_core::app::accessibility_dump::AccessibilityDump;

fn named(role: Role, label: &str, bounds: AccessRect) -> Node {
    let mut node = Node::new(role);
    node.set_label(label);
    node.set_bounds(bounds);
    node
}

#[test]
fn a_plugin_tree_reads_as_a_pane_under_the_window() {
    let mut root = Node::new(Role::Window);
    root.set_label("plugin");
    root.set_children(vec![NodeId(101), NodeId(103)]);
    let mut list = named(Role::List, "Tasks", AccessRect::new(0.0, 0.0, 200.0, 80.0));
    list.set_children(vec![NodeId(102)]);
    let mut checked = named(
        Role::CheckBox,
        "Buy milk",
        AccessRect::new(10.0, 10.0, 130.0, 30.0),
    );
    checked.set_toggled(Toggled::True);
    let hidden = named(
        Role::Button,
        "Hidden",
        AccessRect::new(10.0, 350.0, 70.0, 370.0),
    );
    let mut embedded = Embedded::new(7);
    embedded.update(TreeUpdate {
        nodes: vec![
            (NodeId(100), root),
            (NodeId(101), list),
            (NodeId(102), checked),
            (NodeId(103), hidden),
        ],
        tree: Some(Tree::new(NodeId(100))),
        tree_id: TreeId::ROOT,
        focus: NodeId(102),
    });
    let area = Rect::from_min_size(pos2(100.0, 50.0), vec2(200.0, 100.0));
    let fragment = embedded.fragment(Some("Checklist"), area, area.min.to_vec2(), 1.0, true, true);
    let pane = fragment.root;
    let mut dump = AccessibilityDump::new();
    dump.update(tree_update("Test", VIEWPORT, 2.0, vec![fragment]));
    let rendered = dump.rendered();

    assert_eq!(
        rendered.text(),
        "Window \"Test\" at 0,0 size 800x600\n\
         \x20 Pane \"Checklist\" at 200,100 size 400x200\n\
         \x20   List \"Tasks\" at 200,100 size 400x160\n\
         \x20     CheckBox \"Buy milk\" toggled=True focused at 220,120 size 240x40\n\
         \x20   Button \"Hidden\" at 220,800 size 120x40 offscreen\n"
    );
    let checkbox = rendered
        .lines()
        .iter()
        .find(|line| line.role == Role::CheckBox)
        .unwrap()
        .id;
    assert_eq!(embedded.plugin_node(checkbox), Some(NodeId(102)));
    assert_eq!(embedded.plugin_node(pane), Some(NodeId(100)));
    assert_eq!(embedded.plugin_node(NodeId(102)), None);
}

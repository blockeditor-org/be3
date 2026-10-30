use stable_hash::{HashMap, HashSet};

use accesskit::{Node, NodeId, Role, Tree, TreeId, TreeUpdate};

use super::*;
use beui_core::accessibility::Embedded;

fn with_children(role: Role, children: &[u64]) -> Node {
    let mut node = Node::new(role);
    node.set_children(children.iter().copied().map(NodeId).collect::<Vec<_>>());
    node.set_labelled_by(vec![NodeId(9), NodeId(3)]);
    node
}

#[test]
fn a_plugin_tree_that_is_not_a_tree_is_cut_into_one() {
    let mut embedded = Embedded::new(5);
    embedded.update(TreeUpdate {
        nodes: vec![
            (NodeId(1), with_children(Role::Window, &[2, 3, 2, 9])),
            (NodeId(2), with_children(Role::List, &[1, 3])),
            (NodeId(3), with_children(Role::Button, &[])),
            (NodeId(4), with_children(Role::Button, &[3])),
        ],
        tree: Some(Tree::new(NodeId(1))),
        tree_id: TreeId::ROOT,
        focus: NodeId(9),
    });
    let area = Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 100.0));
    let fragment = embedded.fragment(Some("Broken"), area, Vec2::ZERO, 1.0, true, true);

    let nodes: HashMap<NodeId, &Node> = fragment
        .nodes
        .iter()
        .map(|(id, node)| (*id, node))
        .collect();
    assert_eq!(nodes.len(), 4);
    let mut parented = HashSet::default();
    for node in nodes.values() {
        for child in node.children() {
            assert!(
                nodes.contains_key(child),
                "{child:?} is a child but was not sent"
            );
            assert!(parented.insert(*child), "{child:?} has two parents");
        }
        for labeller in node.labelled_by() {
            assert!(nodes.contains_key(labeller));
        }
    }
    assert!(!parented.contains(&fragment.root));
    assert_eq!(parented.len(), nodes.len() - 1);
    assert_eq!(fragment.focus, None);
}

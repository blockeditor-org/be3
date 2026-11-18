use accesskit::{Tree, TreeId};
use be_lib_geometry::{pos2, vec2};

use super::*;

mod a_restarted_app_s_tree_is_forgotten;
mod an_app_tree_hangs_under_a_pane_where_its_screen_is_drawn;
mod an_app_tree_that_is_not_a_tree_is_cut_into_one;
mod an_app_tree_update_sends_only_what_changed;

fn update(nodes: Vec<(u64, Node)>, root: Option<u64>, focus: u64) -> TreeUpdate {
    TreeUpdate {
        nodes: nodes
            .into_iter()
            .map(|(id, node)| (NodeId(id), node))
            .collect(),
        tree: root.map(|root| Tree::new(NodeId(root))),
        tree_id: TreeId::ROOT,
        focus: NodeId(focus),
    }
}

fn parent(role: Role, children: &[u64]) -> Node {
    let mut node = Node::new(role);
    node.set_children(children.iter().copied().map(NodeId).collect::<Vec<_>>());
    node
}

fn labelled(role: Role, label: &str) -> Node {
    let mut node = Node::new(role);
    node.set_label(label);
    node
}

fn placing(title: Option<&str>, origin: Vec2) -> Placing<'_> {
    Placing {
        title,
        area: Rect::from_min_size(pos2(origin.x, origin.y), vec2(200.0, 100.0)),
        origin,
        scale: 1.0,
    }
}

/// The node sent with this app label, and its host id.
fn sent<'a>(grafted: &'a Grafted, label: &str) -> Option<(NodeId, &'a Node)> {
    grafted
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some(label))
        .map(|(id, node)| (*id, node))
}

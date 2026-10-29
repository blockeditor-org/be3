use accesskit::{Node, NodeId, Rect as AccessRect, Role, Toggled, Tree, TreeId, TreeUpdate};

use super::*;
use beui_core::accessibility::{Embedded, tree_update};
use beui_core::app::accessibility_dump::AccessibilityDump;

fn update(nodes: Vec<(u64, Node)>, root: bool) -> TreeUpdate {
    TreeUpdate {
        nodes: nodes
            .into_iter()
            .map(|(id, node)| (NodeId(id), node))
            .collect(),
        tree: root.then(|| Tree::new(NodeId(1))),
        tree_id: TreeId::ROOT,
        focus: NodeId(1),
    }
}

fn parent(children: &[u64]) -> Node {
    let mut node = Node::new(Role::List);
    node.set_children(children.iter().copied().map(NodeId).collect::<Vec<_>>());
    node
}

fn check(label: &str, toggled: Toggled, top: f64) -> Node {
    let mut node = Node::new(Role::CheckBox);
    node.set_label(label);
    node.set_toggled(toggled);
    node.set_bounds(AccessRect::new(0.0, top, 100.0, top + 20.0));
    node
}

#[test]
fn a_plugin_tree_update_sends_only_what_changed() {
    let area = Rect::from_min_size(pos2(0.0, 0.0), vec2(200.0, 100.0));
    let mut embedded = Embedded::new(3);
    let mut dump = AccessibilityDump::new();
    let mut publish = |embedded: &mut Embedded, origin: Vec2, full: bool| {
        let fragment = embedded.fragment(Some("Tasks"), area, origin, 1.0, false, full);
        let sent = fragment.nodes.len();
        dump.update(tree_update("Test", VIEWPORT, 1.0, vec![fragment]));
        let rendered = dump.rendered();
        let milk = rendered
            .lines()
            .iter()
            .find(|line| line.label == "Milk")
            .map(|line| line.id);
        (sent, rendered.text().to_owned(), milk)
    };
    embedded.update(update(
        vec![(1, parent(&[2])), (2, check("Milk", Toggled::False, 0.0))],
        true,
    ));
    let (sent, _, _) = publish(&mut embedded, Vec2::ZERO, true);
    assert_eq!(sent, 3);

    embedded.update(update(vec![(2, check("Milk", Toggled::True, 0.0))], false));
    let (sent, text, _) = publish(&mut embedded, Vec2::ZERO, false);
    assert_eq!(sent, 1);
    assert!(
        text.contains("CheckBox \"Milk\" toggled=True at 0,0"),
        "{text}"
    );

    let (sent, text, _) = publish(&mut embedded, vec2(0.0, 10.0), false);
    assert_eq!(sent, 2);
    assert!(
        text.contains("CheckBox \"Milk\" toggled=True at 0,10"),
        "{text}"
    );

    embedded.update(update(
        vec![
            (1, parent(&[2, 3])),
            (3, check("Eggs", Toggled::False, 20.0)),
        ],
        false,
    ));
    let (sent, text, milk) = publish(&mut embedded, vec2(0.0, 10.0), false);
    assert_eq!(sent, 4);
    assert!(
        text.contains("CheckBox \"Eggs\" toggled=False at 0,30"),
        "{text}"
    );

    let milk = milk.unwrap();
    embedded.update(update(vec![(1, parent(&[3]))], false));
    let (_, text, _) = publish(&mut embedded, vec2(0.0, 10.0), false);
    assert!(!text.contains("Milk"), "{text}");
    assert_eq!(embedded.plugin_node(milk), None);
}

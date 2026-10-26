use std::rc::Rc;

use accesskit::{Affine, Node, NodeId, Rect, Role, Tree, TreeId, TreeUpdate};

use crate::app::accessibility_dump::AccessibilityDump;

fn label(text: &str, y: f64) -> Node {
    let mut node = Node::new(Role::Label);
    node.set_value(text);
    node.set_bounds(Rect::new(0.0, y, 100.0, y + 10.0));
    node
}

fn update(nodes: Vec<(NodeId, Node)>) -> TreeUpdate {
    TreeUpdate {
        nodes,
        tree: Some(Tree {
            root: NodeId(1),
            toolkit_name: None,
            toolkit_version: None,
        }),
        tree_id: TreeId::ROOT,
        focus: NodeId(1),
    }
}

#[test]
fn a_dump_describes_again_only_the_nodes_that_changed() {
    let mut window = Node::new(Role::Window);
    window.set_bounds(Rect::new(0.0, 0.0, 200.0, 200.0));
    window.set_children(vec![NodeId(2), NodeId(3)]);
    let mut dump = AccessibilityDump::new();
    dump.update(update(vec![
        (NodeId(1), window.clone()),
        (NodeId(2), label("milk", 0.0)),
        (NodeId(3), label("eggs", 20.0)),
    ]));
    let before = dump.rendered();

    dump.update(update(vec![(NodeId(3), label("bread", 20.0))]));
    let after = dump.rendered();
    assert!(Rc::ptr_eq(&before.lines()[1], &after.lines()[1]));
    assert!(!Rc::ptr_eq(&before.lines()[2], &after.lines()[2]));
    assert_eq!(
        after.text(),
        "Window focused at 0,0 size 200x200\n  Label value=\"milk\" at 0,0 size 100x10\n  Label value=\"bread\" at 0,20 size 100x10\n"
    );

    window.set_transform(Affine::translate((5.0, 0.0)));
    dump.update(update(vec![(NodeId(1), window)]));
    assert_eq!(
        dump.rendered().text(),
        "Window focused at 5,0 size 200x200\n  Label value=\"milk\" at 5,0 size 100x10\n  Label value=\"bread\" at 5,20 size 100x10\n"
    );
}

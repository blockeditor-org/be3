use be_lib_layers::WindowLayers;
use be_sdk_api::accesskit::{
    Affine as AccessAffine, Node, NodeId, Rect as AccessRect, Role, Tree, TreeId, TreeUpdate,
};

use super::*;

/// The root's own surface 7, drawn at 2 pixels a point with its top left at 10, 20.
const OWN: LayerId = LayerId(5);

fn own_tree(update: Option<TreeUpdate>) -> Vec<OwnTree> {
    vec![OwnTree {
        surface: 7,
        serial: 1,
        update,
    }]
}

/// A surface's tree as beui describes it: its window node scaled from its points to its pixels,
/// holding a button 4, 6 points in.
fn described() -> TreeUpdate {
    let mut window = Node::new(Role::Window);
    window.set_transform(AccessAffine::scale(2.0));
    window.set_children(vec![NodeId(2)]);
    let mut button = Node::new(Role::Button);
    button.set_bounds(AccessRect::new(4.0, 6.0, 12.0, 14.0));
    TreeUpdate {
        nodes: vec![(NodeId(1), window), (NodeId(2), button)],
        tree: Some(Tree::new(NodeId(1))),
        tree_id: TreeId::ROOT,
        focus: NodeId(2),
    }
}

#[test]
fn the_roots_tree_grafts_each_surface_where_its_layer_shows_it() {
    let mut embedding = Embedding::new(LayerId(0));
    let mut layer = shown_at((10.0, 20.0), 2.0);
    layer.id = OWN;
    layer.surface = Surface::Root(7);
    embedding.drew(LayerList {
        windows: vec![WindowLayers {
            window: None,
            layers: vec![layer],
        }],
    });
    set_describing(false);
    assert!(
        embedding
            .describe(None, vec2(100.0, 100.0), own_tree(Some(described())))
            .is_none(),
        "no tree while the host asks for none"
    );

    set_describing(true);
    let tree = embedding
        .describe(None, vec2(100.0, 100.0), own_tree(Some(described())))
        .expect("a tree while the host asks for one");
    let node = |tree: &TreeUpdate, id: NodeId| {
        tree.nodes
            .iter()
            .find(|(held, _)| *held == id)
            .map(|(_, node)| node.clone())
    };
    let window = node(&tree, NodeId(0)).expect("the window node");
    let [pane] = window.children() else {
        panic!("one pane, for the one layer: {:?}", window.children());
    };
    let pane = node(&tree, *pane).expect("the surface's pane is sent whole the first time");
    let [surface_root] = pane.children() else {
        panic!("the pane holds the surface's tree");
    };
    let surface_root = node(&tree, *surface_root).expect("the surface's root is sent");
    assert_eq!(
        surface_root.transform(),
        Some(
            &(AccessAffine::translate((10.0, 20.0))
                * AccessAffine::scale(0.5)
                * AccessAffine::scale(2.0))
        ),
        "the surface's pixel 0, 0 is at its layer's origin, a pixel half a point"
    );
    assert_eq!(
        tree.focus,
        NodeId(0),
        "the surface's focus counts only while its layer has the keyboard"
    );
    let button = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::Button)
        .map(|(id, _)| *id)
        .expect("the button is sent");

    // With the keyboard, the surface's focused button is the tree's; nothing else changed, so
    // only the window node and the pane go again.
    let _ = embedding.focus(None, Some(OWN));
    let tree = embedding
        .describe(None, vec2(100.0, 100.0), own_tree(None))
        .expect("a tree while the host asks for one");
    assert_eq!(
        tree.focus, button,
        "the focused layer's focus is the tree's"
    );
    assert!(
        tree.nodes.len() <= 2,
        "an unchanged surface sends nothing again: {:?}",
        tree.nodes
    );

    // A surface made in place of another under the same id is a new tree, sent whole.
    let tree = embedding
        .describe(
            None,
            vec2(100.0, 100.0),
            vec![OwnTree {
                surface: 7,
                serial: 2,
                update: Some(described()),
            }],
        )
        .expect("a tree while the host asks for one");
    assert!(
        tree.nodes
            .iter()
            .any(|(_, node)| node.role() == Role::Button),
        "the new surface's tree is sent whole"
    );
    set_describing(false);
}

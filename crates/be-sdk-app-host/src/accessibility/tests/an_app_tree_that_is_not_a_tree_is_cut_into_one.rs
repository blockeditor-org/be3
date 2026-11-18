use super::*;

fn with_children(role: Role, children: &[u64]) -> Node {
    let mut node = parent(role, children);
    node.set_labelled_by(vec![NodeId(9), NodeId(3)]);
    node
}

#[test]
fn an_app_tree_that_is_not_a_tree_is_cut_into_one() {
    let mut graft = Graft::new(5);
    graft.update(update(
        vec![
            (1, with_children(Role::Window, &[2, 3, 2, 9])),
            (2, with_children(Role::List, &[1, 3])),
            (3, with_children(Role::Button, &[])),
            (4, with_children(Role::Button, &[3])),
        ],
        Some(1),
        9,
    ));
    let grafted = graft.grafted(placing(Some("Broken"), Vec2::ZERO), true, true);

    let nodes: HashMap<NodeId, &Node> = grafted
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
    assert!(!parented.contains(&grafted.root));
    assert_eq!(parented.len(), nodes.len() - 1);
    assert_eq!(grafted.focus, None);
}

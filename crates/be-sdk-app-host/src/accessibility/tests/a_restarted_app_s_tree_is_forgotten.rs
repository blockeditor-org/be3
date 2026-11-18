use super::*;

#[test]
fn a_restarted_app_s_tree_is_forgotten() {
    let mut graft = Graft::new(4);
    let tree = || {
        update(
            vec![
                (1, parent(Role::Window, &[2])),
                (2, labelled(Role::Button, "Save")),
            ],
            Some(1),
            2,
        )
    };
    graft.update(tree());
    let grafted = graft.grafted(placing(Some("Notes"), Vec2::ZERO), true, false);
    let (save, _) = sent(&grafted, "Save").unwrap();

    graft.restarted();
    assert_eq!(graft.app_node(save), None);
    let grafted = graft.grafted(placing(Some("Notes"), Vec2::ZERO), true, false);
    let (pane, pane_node) = sent(&grafted, "Notes").expect("the pane is sent again");
    assert_eq!(pane, grafted.root, "the pane keeps its id");
    assert!(
        pane_node.children().is_empty(),
        "until the new run describes its screen, the pane is empty"
    );
    assert_eq!(grafted.nodes.len(), 1);
    assert_eq!(grafted.focus, None);

    graft.update(tree());
    let grafted = graft.grafted(placing(Some("Notes"), Vec2::ZERO), true, false);
    let (again, _) = sent(&grafted, "Save").unwrap();
    assert_ne!(again, save, "the new run's nodes take new host ids");
    assert_eq!(graft.app_node(again), Some(NodeId(2)));
    assert_eq!(grafted.focus, Some(again));
}

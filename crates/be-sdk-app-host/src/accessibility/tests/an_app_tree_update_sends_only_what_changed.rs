use super::*;

#[test]
fn an_app_tree_update_sends_only_what_changed() {
    let mut graft = Graft::new(3);
    graft.update(update(
        vec![
            (1, parent(Role::List, &[2])),
            (2, labelled(Role::CheckBox, "Milk")),
        ],
        Some(1),
        1,
    ));
    let grafted = graft.grafted(placing(Some("Tasks"), Vec2::ZERO), false, true);
    assert_eq!(grafted.nodes.len(), 3, "the pane and both nodes");
    let (milk, _) = sent(&grafted, "Milk").unwrap();

    let mut checked = labelled(Role::CheckBox, "Milk");
    checked.set_toggled(be_sdk_api::accesskit::Toggled::True);
    graft.update(update(vec![(2, checked)], None, 1));
    let grafted = graft.grafted(placing(Some("Tasks"), Vec2::ZERO), false, false);
    assert_eq!(grafted.nodes.len(), 1, "only the node that changed");
    assert_eq!(
        sent(&grafted, "Milk").unwrap().0,
        milk,
        "under the same host id"
    );

    let grafted = graft.grafted(placing(Some("Tasks"), vec2(0.0, 10.0)), false, false);
    assert_eq!(
        grafted.nodes.len(),
        2,
        "a screen that moved sends its pane and the app's root, which carries the move"
    );

    graft.update(update(
        vec![
            (1, parent(Role::List, &[2, 3])),
            (3, labelled(Role::CheckBox, "Eggs")),
        ],
        None,
        1,
    ));
    let grafted = graft.grafted(placing(Some("Tasks"), vec2(0.0, 10.0)), false, false);
    assert_eq!(
        grafted.nodes.len(),
        4,
        "a tree that changed shape is sent whole"
    );

    graft.update(update(vec![(1, parent(Role::List, &[3]))], None, 1));
    let grafted = graft.grafted(placing(Some("Tasks"), vec2(0.0, 10.0)), false, false);
    assert!(sent(&grafted, "Milk").is_none());
    assert_eq!(graft.app_node(milk), None, "a node left out is forgotten");
}

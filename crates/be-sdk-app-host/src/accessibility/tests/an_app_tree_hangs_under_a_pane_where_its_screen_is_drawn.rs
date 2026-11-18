use super::*;

#[test]
fn an_app_tree_hangs_under_a_pane_where_its_screen_is_drawn() {
    let mut graft = Graft::new(7);
    graft.update(update(
        vec![
            (100, parent(Role::Window, &[101])),
            (101, labelled(Role::CheckBox, "Buy milk")),
        ],
        Some(100),
        101,
    ));
    let area = Rect::from_min_size(pos2(100.0, 50.0), vec2(200.0, 100.0));
    let grafted = graft.grafted(
        Placing {
            title: Some("Checklist"),
            area,
            origin: vec2(100.0, 50.0),
            scale: 2.0,
        },
        true,
        true,
    );

    let (pane, pane_node) = sent(&grafted, "Checklist").expect("the pane is sent");
    assert_eq!(pane, grafted.root);
    assert_eq!(pane.0 >> DOCUMENT_SHIFT, 7, "host ids are in the graft's document");
    assert_eq!(pane_node.role(), Role::Pane);
    assert_eq!(
        pane_node.bounds(),
        Some(AccessRect::new(100.0, 50.0, 300.0, 150.0))
    );
    let [root] = pane_node.children() else {
        panic!("the pane holds the app's root");
    };
    let root_node = &grafted.nodes.iter().find(|(id, _)| id == root).unwrap().1;
    assert_eq!(
        root_node.role(),
        Role::GenericContainer,
        "the app's root reads as part of the pane, not a window of its own"
    );
    assert_eq!(
        root_node.transform(),
        Some(&(Affine::translate((100.0, 50.0)) * Affine::scale(2.0))),
        "the app's root is placed where its screen is drawn, at its zoom"
    );

    let (checkbox, _) = sent(&grafted, "Buy milk").unwrap();
    assert_eq!(root_node.children(), [checkbox]);
    assert_eq!(grafted.focus, Some(checkbox));
    assert_eq!(graft.app_node(checkbox), Some(NodeId(101)));
    assert_eq!(graft.app_node(pane), Some(NodeId(100)));
    assert_eq!(
        graft.app_node(NodeId(101)),
        None,
        "the app's own ids are not the host's"
    );

    let unfocused = graft.grafted(
        Placing {
            title: Some("Checklist"),
            area,
            origin: vec2(100.0, 50.0),
            scale: 2.0,
        },
        false,
        false,
    );
    assert_eq!(
        unfocused.focus, None,
        "the app's focus counts only while its screen has the keyboard"
    );
}

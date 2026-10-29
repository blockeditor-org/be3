use super::*;
use crate::native::Document;

const GROUPS: usize = 30;

#[test]
fn an_unchanged_container_is_not_walked_again_for_accessibility_but_a_moved_one_is() {
    let mut document = Document::new();
    let list = document.create_list(Direction::Vertical, 0.0);
    let spacer = document.create_frame();
    document.set_frame_height(spacer, Extent::px(10.0));
    document.append_child(list, spacer.id(), ItemSize::Intrinsic);
    let labels: Vec<NodeOf<TextNode>> = (0..GROUPS)
        .map(|index| {
            let label = document.create_text(format!("group {index}"), 14.0, Color32::WHITE);
            let inner = document.create_list(Direction::Vertical, 0.0);
            document.append_child(inner, label.id(), ItemSize::Intrinsic);
            let group = document.create_frame();
            document.set_frame_child(group, inner.id());
            document.append_child(list, group.id(), ItemSize::Intrinsic);
            label
        })
        .collect();
    document.set_root(list.id());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    harness.document_mut().set_text(labels[3], "group three");
    harness.frame(Vec::new());
    let latest = harness.document().performance().latest;
    assert!(
        latest.work.walked_nodes * 2 < latest.nodes,
        "changing one label walked {} of {} nodes",
        latest.work.walked_nodes,
        latest.nodes
    );

    let before = top_of(&harness, "group 5");
    harness
        .document_mut()
        .set_frame_height(spacer, Extent::px(50.0));
    let moved = harness.frame(Vec::new());
    let sent = moved.accessibility_tree("Test", VIEWPORT).nodes;
    let after = sent
        .iter()
        .find(|(_, node)| node.value() == Some("group 5"))
        .map(|(_, node)| top(node))
        .expect("a label that moved must be sent again");
    assert_eq!(after, before + 40.0);
}

fn top_of(harness: &Harness, value: &str) -> f64 {
    let view = harness.document().accessibility_view().unwrap();
    view.nodes()
        .find(|node| node.value() == Some(value))
        .map(top)
        .unwrap()
}

fn top(node: &accesskit::Node) -> f64 {
    node.transform()
        .map_or(0.0, |transform| transform.as_coeffs()[5])
}

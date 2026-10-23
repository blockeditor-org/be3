use super::*;

const ROWS: usize = 40;

#[test]
fn an_idle_frame_describes_no_accessibility_nodes_and_one_changed_row_describes_few() {
    let mut document = Document::new();
    let rows: Vec<NodeOf<TextNode>> = (0..ROWS)
        .map(|index| document.create_text(format!("row {index}"), 14.0, Color32::WHITE))
        .collect();
    let list = document.create_list(Direction::Vertical, 0.0);
    for row in &rows {
        document.append_child(list, row.id(), ItemSize::Intrinsic);
    }
    let scroll = document.create_offset(list.id());
    document.set_root(scroll.id());
    let mut harness = Harness::new(document);
    let first = harness.frame(Vec::new());
    assert!(first.accessibility_tree("Test", VIEWPORT).nodes.len() > 1);

    let idle = harness.frame(Vec::new());
    assert_eq!(described(&harness), 0);
    assert_eq!(idle.accessibility_tree("Test", VIEWPORT).nodes.len(), 1);

    harness.document_mut().set_text(rows[0], "row nought");
    let changed = harness.frame(Vec::new());
    let work = described(&harness);
    assert!(
        work < ROWS / 4,
        "changing one row described {work} accessibility nodes"
    );
    let sent = changed.accessibility_tree("Test", VIEWPORT).nodes;
    assert!(
        sent.len() < ROWS / 4,
        "changing one row sent {} accessibility nodes",
        sent.len()
    );
    assert!(
        sent.iter()
            .any(|(_, node)| node.value() == Some("row nought"))
    );
}

fn described(harness: &Harness) -> usize {
    harness.document().performance().latest.work.described_nodes
}

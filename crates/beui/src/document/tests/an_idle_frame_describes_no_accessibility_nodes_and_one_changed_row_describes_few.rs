use super::*;
use crate::base::offset::OffsetNode;

const ROWS: usize = 40;

#[test]
fn an_idle_frame_describes_no_accessibility_nodes_and_one_changed_row_describes_few() {
    let mut document = Document::new();
    let scroll = document.create_offset();
    let rows: Vec<NodeId> = (0..ROWS)
        .map(|index| document.create_text(format!("row {index}"), 14.0, Color32::WHITE))
        .collect();
    for row in &rows {
        document
            .arena
            .get_mut_as::<OffsetNode>(scroll)
            .items
            .push(*row);
    }
    document.set_root(scroll);
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

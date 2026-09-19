use super::*;
use crate::base::scroll::ScrollNode;

const ROWS: usize = 40;

#[test]
fn a_scroll_only_re_measures_the_row_that_changed() {
    let mut document = Document::new();
    let scroll = document.create_scroll();
    let rows: Vec<NodeId> = (0..ROWS)
        .map(|index| document.create_text(format!("row {index}"), 14.0, Color32::WHITE))
        .collect();
    for row in &rows {
        document
            .arena
            .get_mut_as::<ScrollNode>(scroll)
            .items
            .push(*row);
    }
    document.set_root(scroll);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.frame(Vec::new());
    let idle = measure_calls(&harness);
    assert_eq!(
        idle, 0,
        "a scroll must not measure its rows on a frame that changed nothing"
    );

    harness.document_mut().set_text(rows[0], "row nought");
    harness.frame(Vec::new());
    let after = measure_calls(&harness);
    assert!(
        after < ROWS / 4,
        "changing one row measured {after} of {ROWS} rows"
    );
}

fn measure_calls(harness: &Harness) -> usize {
    let work = harness.document().performance().latest.work;
    work.measured + work.reused_measurements
}

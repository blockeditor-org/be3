use super::*;

const ROWS: usize = 40;

#[test]
fn a_scroll_only_re_measures_the_row_that_changed() {
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
    work.measured
}

use super::*;
use crate::sight::CULLING_MARGIN;

const ROWS: usize = 500;

#[test]
fn a_list_in_a_scroll_lays_out_only_the_rows_near_the_view() {
    let mut document = Document::new();
    let list = document.create_list(Direction::Vertical, 0.0);
    let rows: Vec<NodeOf<TextNode>> = (0..ROWS)
        .map(|index| document.create_text(format!("row {index}"), 14.0, Color32::WHITE))
        .collect();
    for row in &rows {
        document.append_child(list, row.id(), ItemSize::Intrinsic);
    }
    let scroll = document.create_offset(list.id());
    document.set_root(scroll.id());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let placed = |harness: &Harness| -> Vec<usize> {
        (0..ROWS)
            .filter(|index| !harness.document().is_culled(rows[*index].id()))
            .collect()
    };
    let row_height = harness.rect(rows[1]).top() - harness.rect(rows[0]).top();
    let reach = VIEWPORT.y + CULLING_MARGIN;
    let shown = placed(&harness);
    assert_eq!(shown.first(), Some(&0));
    assert!(
        shown.len() < ROWS / 4,
        "{} of {ROWS} rows were laid out with only the top of the list in view",
        shown.len()
    );
    assert!(
        (shown.len() as f32 * row_height) >= reach,
        "the rows laid out must reach past the view by the margin"
    );
    assert!(
        harness.document().node_rect(rows[ROWS - 1]).is_some(),
        "a row left out still keeps its place"
    );

    let offset = 200.0 * row_height;
    harness
        .document_mut()
        .set_scroll_offset(scroll.id(), offset);
    harness.frame(Vec::new());
    let shown = placed(&harness);
    let in_view = (offset / row_height) as usize;
    assert!(
        !shown.contains(&0) && shown.contains(&in_view) && !shown.contains(&(ROWS - 1)),
        "scrolled to row {in_view}, the rows laid out were {shown:?}"
    );
    assert_eq!(
        harness
            .document()
            .node_rect(rows[in_view])
            .map(|rect| rect.top()),
        Some(0.0),
        "the row scrolled to sits at the top of the view"
    );

    harness
        .document_mut()
        .set_scroll_offset(scroll.id(), offset + 1.0);
    harness.frame(Vec::new());
    let work = harness.document().performance().latest.work;
    assert!(
        work.placed <= 2,
        "scrolling a pixel that brings no row in or out laid out {} nodes",
        work.placed
    );
    assert_eq!(placed(&harness), shown);
}

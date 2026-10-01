use super::*;
use crate::reactive::NodeRef;

#[test]
fn overscrolling_a_sheet_bounces_its_content_back() {
    let sheet = NodeRef::new();
    let closed = Rc::new(Cell::new(0));
    let document = sheet_of_rows(0.9, &sheet, &closed);
    let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
    harness.settle();
    let sheet = sheet.get();
    let content = SHEET_ROWS as f32 * SHEET_ROW_HEIGHT;

    harness.touch(TouchPhase::Start, pos2(200.0, 580.0));
    for step in 1..=60 {
        harness.touch(TouchPhase::Move, pos2(200.0, 580.0 - step as f32 * 15.0));
    }
    let end = harness.document().scroll_offset(sheet);
    let stretched = harness.document().scroll_overscroll(sheet);
    assert_eq!(
        harness.rect(sheet).height(),
        540.0,
        "the sheet stays at its top stop"
    );
    assert!(
        end > content - 540.0,
        "the content scrolls to its end, at {end}"
    );
    assert!(
        stretched > 0.0 && stretched < 300.0,
        "and pulling past it stretches the content a little, by {stretched}"
    );

    harness.touch(TouchPhase::End, pos2(200.0, -320.0));
    harness.frame(Vec::new());
    assert!(
        harness.document().scroll_overscroll(sheet) < stretched,
        "let go, the content springs back"
    );
    harness.settle();
    assert_eq!(harness.document().scroll_overscroll(sheet), 0.0);
    assert_eq!(
        harness.document().scroll_offset(sheet),
        end,
        "and rests at its end"
    );
}
